//! 任务类型路由：查 route_policy 决策调用链（决策 ≤100ms，N-06）。
//!
//! 决策过程：task_type → route_policy（primary + fallbacks）
//! → 逐目标解析 provider（启用 + 有密钥 + 预算未超）→ 产出有序调用链。

use crate::db::dao::model::{ModelDao, ModelProvider, RoutePolicy};
use crate::db::DbPool;
use crate::error::AppError;

/// 路由目标（调用链的一环）。
#[derive(Debug, Clone)]
pub struct RouteTarget {
    /// "provider.model" 全名（熔断器键）
    pub full_name: String,
    pub provider: ModelProvider,
    pub model: String,
    pub timeout_ms: i64,
}

/// 路由器。
pub struct Router {
    db: DbPool,
}

impl Router {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    /// 解析任务类型的有序调用链。
    ///
    /// 预算策略：policy 配置月度预算时，月累计成本超限的 provider 被跳过；
    /// 全链超限返回 4002。
    pub fn resolve(&self, task_type: &str) -> Result<Vec<RouteTarget>, AppError> {
        let conn = self.db.conn()?;
        let dao = ModelDao::new(&conn);
        let policy: RoutePolicy = dao
            .policy_for(task_type)?
            .ok_or_else(|| AppError::invalid_param(format!("未配置任务类型 {task_type} 的路由策略")))?;

        let mut chain: Vec<String> = vec![policy.primary.clone()];
        chain.extend(policy.fallbacks.clone());

        let mut targets = Vec::new();
        let mut budget_blocked = 0usize;
        for item in &chain {
            let (provider_name, model) = match item.split_once('.') {
                Some((p, m)) if !p.is_empty() && !m.is_empty() => (p.to_string(), m.to_string()),
                _ => {
                    tracing::warn!("路由目标格式非法（期望 provider.model）: {item}");
                    continue;
                }
            };
            let provider = match dao.provider_by_name(&provider_name)? {
                Some(p) => p,
                None => {
                    tracing::warn!("路由目标 provider 未配置或未启用: {provider_name}");
                    continue;
                }
            };
            // 月度预算检查（F-19）
            if let Some(budget) = policy.monthly_budget_cny {
                let spend = dao.monthly_spend(&provider_name)?;
                if spend >= budget {
                    tracing::warn!(
                        "provider {provider_name} 月成本 {spend:.1} 分已达预算 {budget:.1} 分，跳过"
                    );
                    budget_blocked += 1;
                    continue;
                }
            }
            targets.push(RouteTarget {
                full_name: item.clone(),
                provider,
                model,
                timeout_ms: policy.timeout_ms,
            });
        }

        if targets.is_empty() {
            if budget_blocked > 0 && budget_blocked == chain.len() {
                return Err(AppError::budget_exceeded(
                    "全部候选模型月度预算均已超限，请调整预算或等待下月",
                ));
            }
            return Err(AppError::ai_unavailable(format!(
                "任务类型 {task_type} 无可用模型（请检查模型配置与密钥）"
            )));
        }
        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::dao::model::{ModelSpec, ProviderInput};

    fn pool(tag: &str) -> DbPool {
        DbPool::test_pool(tag).unwrap()
    }

    fn add_provider(pool: &DbPool, name: &str, enabled: bool) {
        let conn = pool.conn().unwrap();
        ModelDao::new(&conn)
            .upsert_provider(
                &ProviderInput {
                    id: None,
                    name: name.into(),
                    protocol: None,
                    base_url: "https://example.com/v1".into(),
                    api_key: None,
                    models: vec![ModelSpec {
                        id: format!("{name}-chat"),
                        price_in: 0.1,
                        price_out: 0.2,
                        ctx: 8192,
                    }],
                    enabled: Some(enabled),
                    priority: None,
                },
                &format!("provider-{name}"),
            )
            .unwrap();
    }

    #[test]
    fn resolve_returns_primary_then_fallbacks_in_order() {
        let pool = pool("router-order");
        add_provider(&pool, "deepseek", true);
        add_provider(&pool, "glm", true);
        add_provider(&pool, "kimi", true);
        let router = Router::new(pool);
        // 迁移内置策略：ocr_attribution → deepseek + [glm, kimi]
        let targets = router.resolve("ocr_attribution").unwrap();
        let names: Vec<&str> = targets.iter().map(|t| t.full_name.as_str()).collect();
        assert_eq!(
            names,
            vec!["deepseek.deepseek-chat", "glm.glm-4-flash", "kimi.moonshot-v1-8k"],
            "调用链顺序 = primary + fallbacks"
        );
        assert_eq!(targets[0].timeout_ms, 30000);
    }

    #[test]
    fn unknown_task_type_rejected_1001() {
        let pool = pool("router-unknown");
        let router = Router::new(pool);
        let err = router.resolve("not_a_task").unwrap_err();
        assert_eq!(err.code, 1001);
    }

    #[test]
    fn unconfigured_provider_skipped() {
        let pool = pool("router-skip");
        // 只配置 deepseek，glm/kimi 未配置 → 被跳过
        add_provider(&pool, "deepseek", true);
        let router = Router::new(pool);
        let targets = router.resolve("ocr_attribution").unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].full_name, "deepseek.deepseek-chat");
    }

    #[test]
    fn disabled_provider_skipped() {
        let pool = pool("router-disabled");
        add_provider(&pool, "deepseek", false);
        let router = Router::new(pool);
        let err = router.resolve("ocr_attribution").unwrap_err();
        assert_eq!(err.code, 4001, "全部 provider 停用/缺失 → 4001");
    }

    #[test]
    fn all_budget_exceeded_returns_4002() {
        let pool = pool("router-budget");
        add_provider(&pool, "deepseek", true);
        {
            let conn = pool.conn().unwrap();
            let dao = ModelDao::new(&conn);
            // 把 ocr_attribution 策略改为预算 0 且只含 deepseek
            dao.upsert_policy(&RoutePolicy {
                id: 0,
                task_type: "ocr_attribution".into(),
                primary: "deepseek.deepseek-chat".into(),
                fallbacks: vec![],
                timeout_ms: 30000,
                max_cost_cny: None,
                monthly_budget_cny: Some(0.0),
            })
            .unwrap();
            // 造一笔本月成本
            conn.execute(
                "INSERT INTO call_log(provider, model, task_type, cost_cny, status)
                 VALUES ('deepseek', 'deepseek-chat', 'ocr_attribution', 1.0, 'ok')",
                [],
            )
            .unwrap();
        }
        let router = Router::new(pool);
        let err = router.resolve("ocr_attribution").unwrap_err();
        assert_eq!(err.code, 4002, "全链预算超限应返回 4002");
    }

    #[test]
    fn malformed_chain_entry_skipped() {
        let pool = pool("router-malformed");
        add_provider(&pool, "deepseek", true);
        {
            let conn = pool.conn().unwrap();
            ModelDao::new(&conn)
                .upsert_policy(&RoutePolicy {
                    id: 0,
                    task_type: "ocr_attribution".into(),
                    primary: "bad-format-no-dot".into(),
                    fallbacks: vec!["deepseek.deepseek-chat".into()],
                    timeout_ms: 30000,
                    max_cost_cny: None,
                    monthly_budget_cny: None,
                })
                .unwrap();
        }
        let router = Router::new(pool);
        let targets = router.resolve("ocr_attribution").unwrap();
        assert_eq!(targets.len(), 1, "非法格式条目应被跳过");
        assert_eq!(targets[0].full_name, "deepseek.deepseek-chat");
    }
}
