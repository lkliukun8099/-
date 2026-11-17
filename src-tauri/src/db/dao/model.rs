//! 模型接入 DAO：provider / 路由策略 / 预算查询。

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 单个模型规格（价格单位：分/千 token）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpec {
    pub id: String,
    #[serde(default)]
    pub price_in: f64,
    #[serde(default)]
    pub price_out: f64,
    #[serde(default)]
    pub ctx: i64,
}

/// 模型供应商配置（API Key 不落库，仅存 keyring 别名）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProvider {
    pub id: i64,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub key_alias: String,
    pub models: Vec<ModelSpec>,
    pub enabled: bool,
    pub priority: i64,
    /// 前端展示用：钥匙串中是否已配置密钥（不回传密钥本体）
    #[serde(default)]
    pub has_key: bool,
}

/// 新建/更新 provider 的输入。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInput {
    pub id: Option<i64>,
    pub name: String,
    pub protocol: Option<String>,
    pub base_url: String,
    /// 明文密钥仅在此入口出现一次，写入 keyring 后即弃
    pub api_key: Option<String>,
    pub models: Vec<ModelSpec>,
    pub enabled: Option<bool>,
    pub priority: Option<i64>,
}

/// 路由策略。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePolicy {
    pub id: i64,
    pub task_type: String,
    pub primary: String,
    pub fallbacks: Vec<String>,
    pub timeout_ms: i64,
    pub max_cost_cny: Option<f64>,
    pub monthly_budget_cny: Option<f64>,
}

pub struct ModelDao<'a> {
    conn: &'a Connection,
}

impl<'a> ModelDao<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_provider(row: &Row) -> rusqlite::Result<ModelProvider> {
        let models_json: String = row.get("models")?;
        Ok(ModelProvider {
            id: row.get("id")?,
            name: row.get("name")?,
            protocol: row.get("protocol")?,
            base_url: row.get("base_url")?,
            key_alias: row.get("key_alias")?,
            models: serde_json::from_str(&models_json).unwrap_or_default(),
            enabled: row.get::<_, i64>("enabled")? != 0,
            priority: row.get("priority")?,
            has_key: false, // 由 service 层查 keyring 回填
        })
    }

    fn row_to_policy(row: &Row) -> rusqlite::Result<RoutePolicy> {
        let fallbacks_json: String = row.get("fallbacks")?;
        Ok(RoutePolicy {
            id: row.get("id")?,
            task_type: row.get("task_type")?,
            primary: row.get("primary")?,
            fallbacks: serde_json::from_str(&fallbacks_json).unwrap_or_default(),
            timeout_ms: row.get("timeout_ms")?,
            max_cost_cny: row.get("max_cost_cny")?,
            monthly_budget_cny: row.get("monthly_budget_cny")?,
        })
    }

    // ---- provider ----

    pub fn list_providers(&self) -> Result<Vec<ModelProvider>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM model_provider ORDER BY priority DESC, id")?;
        let rows = stmt
            .query_map([], Self::row_to_provider)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 按 ID 取 provider（含停用）。
    pub fn provider_by_id(&self, id: i64) -> Result<Option<ModelProvider>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM model_provider WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], Self::row_to_provider)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 按名称取启用中的 provider（路由解析用）。
    pub fn provider_by_name(&self, name: &str) -> Result<Option<ModelProvider>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM model_provider WHERE name = ?1 AND enabled = 1")?;
        let mut rows = stmt.query_map(params![name], Self::row_to_provider)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 新建或更新 provider（id 存在则更新）。
    pub fn upsert_provider(&self, input: &ProviderInput, key_alias: &str) -> Result<i64, AppError> {
        let models_json = serde_json::to_string(&input.models)?;
        let protocol = input.protocol.as_deref().unwrap_or("openai_compat");
        match input.id {
            Some(id) => {
                let affected = self.conn.execute(
                    "UPDATE model_provider SET name=?2, protocol=?3, base_url=?4,
                       models=?5, enabled=COALESCE(?6, enabled),
                       priority=COALESCE(?7, priority)
                     WHERE id=?1",
                    params![
                        id,
                        input.name,
                        protocol,
                        input.base_url,
                        models_json,
                        input.enabled.map(|b| b as i64),
                        input.priority,
                    ],
                )?;
                if affected == 0 {
                    return Err(AppError::not_found(format!("模型供应商 #{id} 不存在")));
                }
                Ok(id)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO model_provider(name, protocol, base_url, key_alias, models,
                       enabled, priority)
                     VALUES (?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        input.name,
                        protocol,
                        input.base_url,
                        key_alias,
                        models_json,
                        input.enabled.unwrap_or(true) as i64,
                        input.priority.unwrap_or(0),
                    ],
                )?;
                Ok(self.conn.last_insert_rowid())
            }
        }
    }

    pub fn delete_provider(&self, id: i64) -> Result<(), AppError> {
        let affected = self
            .conn
            .execute("DELETE FROM model_provider WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(AppError::not_found(format!("模型供应商 #{id} 不存在")));
        }
        Ok(())
    }

    // ---- 路由策略 ----

    pub fn list_policies(&self) -> Result<Vec<RoutePolicy>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM route_policy ORDER BY id")?;
        let rows = stmt
            .query_map([], Self::row_to_policy)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 按任务类型取路由策略（路由决策核心查询，走 UNIQUE 索引）。
    pub fn policy_for(&self, task_type: &str) -> Result<Option<RoutePolicy>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM route_policy WHERE task_type = ?1")?;
        let mut rows = stmt.query_map(params![task_type], Self::row_to_policy)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn upsert_policy(&self, policy: &RoutePolicy) -> Result<i64, AppError> {
        let fallbacks_json = serde_json::to_string(&policy.fallbacks)?;
        self.conn.execute(
            "INSERT INTO route_policy(task_type, primary, fallbacks, timeout_ms,
               max_cost_cny, monthly_budget_cny)
             VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(task_type) DO UPDATE SET
               primary = excluded.primary,
               fallbacks = excluded.fallbacks,
               timeout_ms = excluded.timeout_ms,
               max_cost_cny = excluded.max_cost_cny,
               monthly_budget_cny = excluded.monthly_budget_cny",
            params![
                policy.task_type,
                policy.primary,
                fallbacks_json,
                policy.timeout_ms,
                policy.max_cost_cny,
                policy.monthly_budget_cny,
            ],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM route_policy WHERE task_type = ?1",
            params![policy.task_type],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    /// 本月某 provider 已产生成本（预算检查用，单位：分）。
    pub fn monthly_spend(&self, provider: &str) -> Result<f64, AppError> {
        let spend: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(cost_cny), 0) FROM call_log
             WHERE provider = ?1 AND strftime('%Y-%m', created_at) = strftime('%Y-%m', 'now')",
            params![provider],
            |r| r.get(0),
        )?;
        Ok(spend)
    }
}
