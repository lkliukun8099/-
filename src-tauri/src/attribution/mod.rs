//! 归因引擎（F-06）：规则库优先 → LLM 深判 → 版本化冲突合并（AC-8）。
//!
//! 模式：
//! - local：仅规则库初判（断网路径，结果标记 origin=local，错题保持 pending_analysis）
//! - cloud：LLM 深度归因并与本地初判比对，一致则替换、不一致生成冲突
//! - auto：在线走 cloud，离线走 local

pub mod prompts;
pub mod rules;

use std::sync::Arc;

use rules::RuleEngine;

use crate::ai::adapter::{ChatMessage, ChatRequest};
use crate::ai::Gateway;
use crate::db::dao::attribution::{Attribution, AttributionConfirm, AttributionDao, NewAttribution};
use crate::db::dao::kg::KgDao;
use crate::db::dao::mistake::MistakeDao;
use crate::db::DbPool;
use crate::error::AppError;
use crate::kg;

/// 归因引擎。
pub struct AttributionEngine {
    db: DbPool,
    gateway: Arc<Gateway>,
    rules: RuleEngine,
}

/// 归因运行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributionMode {
    Local,
    Cloud,
    Auto,
}

impl AttributionMode {
    pub fn parse(s: &str, online: bool) -> Result<Self, AppError> {
        match s {
            "local" => Ok(Self::Local),
            "cloud" => Ok(Self::Cloud),
            "auto" => Ok(if online { Self::Cloud } else { Self::Local }),
            _ => Err(AppError::invalid_param(format!(
                "归因模式仅支持 local/cloud/auto，收到: {s}"
            ))),
        }
    }
}

impl AttributionEngine {
    pub fn new(db: DbPool, gateway: Arc<Gateway>) -> Self {
        Self {
            db,
            gateway,
            rules: RuleEngine::new(),
        }
    }

    /// 主入口：对错题运行归因，返回生效的归因版本。
    pub async fn analyze(&self, mistake_id: i64, mode: AttributionMode) -> Result<Attribution, AppError> {
        let mistake = {
            let conn = self.db.conn()?;
            MistakeDao::new(&conn).get(mistake_id)?
        };
        {
            let conn = self.db.conn()?;
            MistakeDao::new(&conn).update_status(mistake_id, "analyzing")?;
        }

        let result = match mode {
            AttributionMode::Local => self.analyze_local(&mistake).await,
            AttributionMode::Cloud => self.analyze_cloud(&mistake).await,
            AttributionMode::Auto => unreachable!("auto 已在 parse 阶段展开"),
        };

        if let Err(e) = &result {
            // 失败回滚状态，允许重试
            let conn = self.db.conn()?;
            let _ = MistakeDao::new(&conn).update_status(mistake_id, "pending_analysis");
            return Err(AppError::attribution(e.message.clone()).with_detail(
                e.detail.clone().unwrap_or_default(),
            ));
        }
        result
    }

    /// 本地规则初判。
    async fn analyze_local(&self, mistake: &crate::db::dao::mistake::Mistake) -> Result<Attribution, AppError> {
        let stem = mistake.ocr_text.clone().unwrap_or_default();
        let answer = mistake.student_answer.clone().unwrap_or_default();
        let hit = self.rules.evaluate(&stem, &answer);

        let conn = self.db.conn()?;
        let dao = AttributionDao::new(&conn);
        let kg_dao = KgDao::new(&conn);

        let (label, detail, kg_node_id, confidence) = match hit {
            Some(h) => {
                let node_id = kg_dao
                    .get_by_code(1, &h.kg_code)? // 1 = 数学（迁移 V1 固定）
                    .map(|n| n.id);
                (h.cause_label, Some(h.cause_detail), node_id, h.confidence)
            }
            None => (
                "规则库未命中，待联网深度分析".to_string(),
                Some("本地规则库未能识别错因模式，联网后将自动提交云端深度归因。".to_string()),
                None,
                0.2,
            ),
        };

        // 本地初判与既有生效版本不重复
        if let Some(active) = dao.active_for_mistake(mistake.id)? {
            if active.origin == "local" && active.cause_label == label {
                let conn2 = self.db.conn()?;
                let _ = MistakeDao::new(&conn2).update_status(mistake.id, "pending_analysis");
                return Ok(active);
            }
        }

        let id = dao.insert(&NewAttribution {
            mistake_id: mistake.id,
            origin: "local".into(),
            engine: "rule".into(),
            cause_label: label,
            cause_detail: detail,
            kg_node_id,
            confidence,
            is_active: true,
        })?;
        // 本地初判后保持"待深度分析"，联网后批量深判（场景 C）
        MistakeDao::new(&conn).update_status(mistake.id, "pending_analysis")?;
        let mut attr = dao.get(id)?;
        attr.kg_path = kg_node_id.map(|nid| kg::node_path(&conn, nid)).unwrap_or_default();
        Ok(attr)
    }

    /// 云端 LLM 深判 + 冲突检测。
    async fn analyze_cloud(&self, mistake: &crate::db::dao::mistake::Mistake) -> Result<Attribution, AppError> {
        // 图谱候选：取数学全部三级节点（编码+名称），供 LLM 定位
        let kg_candidates: Vec<(String, String)> = {
            let conn = self.db.conn()?;
            let nodes = KgDao::new(&conn).all_for_subject(1)?;
            nodes
                .into_iter()
                .filter(|n| n.level == 3)
                .map(|n| (n.code, n.name))
                .collect()
        };

        let prompt = prompts::build_attribution_prompt(mistake, &kg_candidates);
        let req = ChatRequest {
            model: String::new(),
            messages: vec![
                ChatMessage::system(prompts::ATTRIBUTION_SYSTEM),
                ChatMessage::user(prompt),
            ],
            temperature: Some(0.2),
            max_tokens: Some(800),
        };
        let resp = self.gateway.chat_once("ocr_attribution", req).await?;
        let json_str = prompts::extract_json(&resp.content)?;
        let parsed: prompts::LlmAttribution = serde_json::from_str(json_str)
            .map_err(|e| AppError::attribution(format!("归因 JSON 解析失败: {e}")))?;

        let conn = self.db.conn()?;
        let dao = AttributionDao::new(&conn);
        let kg_dao = KgDao::new(&conn);

        let kg_node_id = match &parsed.kg_code {
            Some(code) => kg_dao.get_by_code(1, code)?.map(|n| n.id),
            None => None,
        };

        let cloud_id = dao.insert(&NewAttribution {
            mistake_id: mistake.id,
            origin: "cloud".into(),
            engine: "llm".into(),
            cause_label: parsed.cause_label.clone(),
            cause_detail: parsed.cause_detail.clone(),
            kg_node_id,
            confidence: parsed.confidence.clamp(0.0, 1.0),
            is_active: false, // 默认先生效与否取决于与本地初判的比对
        })?;

        // 与本地初判比对（场景 C / AC-8）
        let local_active = dao
            .list_for_mistake(mistake.id)?
            .into_iter()
            .find(|a| a.origin == "local" && a.is_active);

        match local_active {
            Some(local) => {
                let consistent =
                    local.cause_label == parsed.cause_label && local.kg_node_id == kg_node_id;
                if consistent {
                    // 一致：云端生效，本地降级保留
                    dao.activate(cloud_id)?;
                    dao.supersede(local.id, cloud_id)?;
                    MistakeDao::new(&conn).update_status(mistake.id, "analyzed")?;
                } else {
                    // 不一致：登记冲突，错题进入 conflict 状态
                    dao.insert_conflict(mistake.id, local.id, cloud_id)?;
                    MistakeDao::new(&conn).update_status(mistake.id, "conflict")?;
                    tracing::info!("归因冲突登记: mistake={} local=#{} cloud=#{}",
                        mistake.id, local.id, cloud_id);
                }
            }
            None => {
                // 无本地初判：云端直接生效
                dao.activate(cloud_id)?;
                MistakeDao::new(&conn).update_status(mistake.id, "analyzed")?;
            }
        }

        let mut attr = dao.get(cloud_id)?;
        attr.kg_path = kg_node_id.map(|nid| kg::node_path(&conn, nid)).unwrap_or_default();
        Ok(attr)
    }

    /// 用户确认/修改归因（G2 埋点：修改比例即"不认可率"）。
    pub fn confirm(&self, input: AttributionConfirm) -> Result<(), AppError> {
        let conn = self.db.conn()?;
        let dao = AttributionDao::new(&conn);
        let original = dao.get(input.attribution_id)?;

        let edited = input.edited_label.is_some()
            || input.edited_detail.is_some()
            || input.edited_kg_node_id.is_some();

        if edited {
            // 修改：生成 origin=user 新版本生效，原版本降级保留
            let new_id = dao.insert(&NewAttribution {
                mistake_id: original.mistake_id,
                origin: "user".into(),
                engine: "manual".into(),
                cause_label: input
                    .edited_label
                    .clone()
                    .unwrap_or_else(|| original.cause_label.clone()),
                cause_detail: input
                    .edited_detail
                    .clone()
                    .or_else(|| original.cause_detail.clone()),
                kg_node_id: input.edited_kg_node_id.or(original.kg_node_id),
                confidence: 1.0,
                is_active: true,
            })?;
            dao.supersede(original.id, new_id)?;
            bump_counter(&conn, "g2_attribution_edited");
        } else {
            bump_counter(&conn, "g2_attribution_confirmed");
        }
        MistakeDao::new(&conn).update_status(original.mistake_id, "analyzed")?;
        Ok(())
    }

    /// 裁决冲突（choice: local / cloud / merged）。
    pub fn resolve_conflict(
        &self,
        conflict_id: i64,
        choice: &str,
        merged_label: Option<String>,
        merged_detail: Option<String>,
    ) -> Result<(), AppError> {
        let conn = self.db.conn()?;
        let dao = AttributionDao::new(&conn);
        let conflicts = dao.list_conflicts(Some("pending"))?;
        let conflict = conflicts
            .into_iter()
            .find(|c| c.id == conflict_id)
            .ok_or_else(|| AppError::not_found(format!("冲突 #{conflict_id} 不存在或已处理")))?;

        match choice {
            "local" => {
                dao.activate(conflict.local_attr.id)?;
                dao.supersede(conflict.cloud_attr.id, conflict.local_attr.id)?;
            }
            "cloud" => {
                dao.activate(conflict.cloud_attr.id)?;
                dao.supersede(conflict.local_attr.id, conflict.cloud_attr.id)?;
            }
            "merged" => {
                let label = merged_label
                    .ok_or_else(|| AppError::invalid_param("合并裁决需要提供 merged_label"))?;
                let merged_id = dao.insert(&NewAttribution {
                    mistake_id: conflict.entity_id,
                    origin: "user".into(),
                    engine: "merged".into(),
                    cause_label: label,
                    cause_detail: merged_detail,
                    kg_node_id: conflict
                        .cloud_attr
                        .kg_node_id
                        .or(conflict.local_attr.kg_node_id),
                    confidence: 1.0,
                    is_active: true,
                })?;
                dao.supersede(conflict.local_attr.id, merged_id)?;
                dao.supersede(conflict.cloud_attr.id, merged_id)?;
            }
            _ => return Err(AppError::invalid_param("choice 仅支持 local/cloud/merged")),
        }

        dao.resolve_conflict(conflict_id, choice)?;
        MistakeDao::new(&conn).update_status(conflict.entity_id, "analyzed")?;
        Ok(())
    }
}

/// G2 指标计数器（app_meta 累加）。
fn bump_counter(conn: &rusqlite::Connection, key: &str) {
    let _ = conn.execute(
        "INSERT INTO app_meta(key, value) VALUES (?1, '1')
         ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT)",
        [key],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::dao::attribution::NewAttribution;
    use crate::db::dao::mistake::MistakeDao;

    struct Fixture {
        engine: AttributionEngine,
        db: DbPool,
        mistake_id: i64,
        local_id: i64,
        cloud_id: i64,
        conflict_id: i64,
    }

    /// 造场景 C 冲突现场：本地初判生效 + 云端深判不一致 + pending 冲突。
    fn fixture(tag: &str) -> Fixture {
        let db = DbPool::test_pool(tag).unwrap();
        let gateway = Arc::new(Gateway::new(db.clone()));
        let engine = AttributionEngine::new(db.clone(), gateway);
        let conn = db.conn().unwrap();
        let mistake_id = MistakeDao::new(&conn).insert(1, "images/t.png").unwrap();
        let dao = AttributionDao::new(&conn);
        let local_id = dao
            .insert(&NewAttribution {
                mistake_id,
                origin: "local".into(),
                engine: "rule".into(),
                cause_label: "本地初判标签".into(),
                cause_detail: None,
                kg_node_id: None,
                confidence: 0.6,
                is_active: true,
            })
            .unwrap();
        let cloud_id = dao
            .insert(&NewAttribution {
                mistake_id,
                origin: "cloud".into(),
                engine: "llm".into(),
                cause_label: "云端深判标签".into(),
                cause_detail: None,
                kg_node_id: None,
                confidence: 0.9,
                is_active: false,
            })
            .unwrap();
        let conflict_id = dao.insert_conflict(mistake_id, local_id, cloud_id).unwrap();
        drop(conn);
        Fixture {
            engine,
            db,
            mistake_id,
            local_id,
            cloud_id,
            conflict_id,
        }
    }

    #[test]
    fn conflict_insert_dedup_pending() {
        let f = fixture("attr-dedup");
        let conn = f.db.conn().unwrap();
        let dao = AttributionDao::new(&conn);
        let again = dao.insert_conflict(f.mistake_id, f.local_id, f.cloud_id).unwrap();
        assert_eq!(again, f.conflict_id, "同错题已有 pending 冲突时不重复登记");
    }

    #[test]
    fn resolve_cloud_activates_cloud_and_keeps_both_versions() {
        // AC-8：裁决后选定方生效，另一方 superseded 但记录保留
        let f = fixture("attr-cloud");
        f.engine
            .resolve_conflict(f.conflict_id, "cloud", None, None)
            .unwrap();
        let conn = f.db.conn().unwrap();
        let dao = AttributionDao::new(&conn);
        let cloud = dao.get(f.cloud_id).unwrap();
        let local = dao.get(f.local_id).unwrap();
        assert!(cloud.is_active);
        assert!(!local.is_active);
        assert_eq!(local.superseded_by, Some(f.cloud_id), "双版本保留并可追溯");
        // 冲突状态与错题状态
        let conflicts = dao.list_conflicts(Some("resolved")).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].resolved_choice.as_deref(), Some("cloud"));
        let m = MistakeDao::new(&conn).get(f.mistake_id).unwrap();
        assert_eq!(m.status, "analyzed");
    }

    #[test]
    fn resolve_local_activates_local() {
        let f = fixture("attr-local");
        f.engine
            .resolve_conflict(f.conflict_id, "local", None, None)
            .unwrap();
        let conn = f.db.conn().unwrap();
        let dao = AttributionDao::new(&conn);
        assert!(dao.get(f.local_id).unwrap().is_active);
        let cloud = dao.get(f.cloud_id).unwrap();
        assert!(!cloud.is_active);
        assert_eq!(cloud.superseded_by, Some(f.local_id));
    }

    #[test]
    fn resolve_merged_creates_user_version() {
        let f = fixture("attr-merged");
        f.engine
            .resolve_conflict(
                f.conflict_id,
                "merged",
                Some("合并后的错因标签".into()),
                Some("综合本地与云端".into()),
            )
            .unwrap();
        let conn = f.db.conn().unwrap();
        let dao = AttributionDao::new(&conn);
        let active = dao.active_for_mistake(f.mistake_id).unwrap().unwrap();
        assert_eq!(active.origin, "user");
        assert_eq!(active.engine, "merged");
        assert_eq!(active.cause_label, "合并后的错因标签");
        assert_eq!(active.confidence, 1.0);
        // 双版本均失效但保留
        assert!(!dao.get(f.local_id).unwrap().is_active);
        assert!(!dao.get(f.cloud_id).unwrap().is_active);
        assert_eq!(dao.list_for_mistake(f.mistake_id).unwrap().len(), 3);
    }

    #[test]
    fn resolve_merged_without_label_rejected() {
        let f = fixture("attr-merged-noname");
        let err = f
            .engine
            .resolve_conflict(f.conflict_id, "merged", None, None)
            .unwrap_err();
        assert_eq!(err.code, 1001);
    }

    #[test]
    fn resolve_invalid_choice_rejected() {
        let f = fixture("attr-badchoice");
        let err = f
            .engine
            .resolve_conflict(f.conflict_id, "coin_flip", None, None)
            .unwrap_err();
        assert_eq!(err.code, 1001);
    }

    #[test]
    fn resolve_twice_rejected() {
        let f = fixture("attr-twice");
        f.engine
            .resolve_conflict(f.conflict_id, "cloud", None, None)
            .unwrap();
        let err = f
            .engine
            .resolve_conflict(f.conflict_id, "local", None, None)
            .unwrap_err();
        assert_eq!(err.code, 1002, "已裁决冲突不可重复裁决");
    }

    #[test]
    fn confirm_without_edit_marks_confirmed_counter() {
        let f = fixture("attr-confirm");
        f.engine
            .confirm(AttributionConfirm {
                attribution_id: f.local_id,
                edited_label: None,
                edited_detail: None,
                edited_kg_node_id: None,
            })
            .unwrap();
        let conn = f.db.conn().unwrap();
        let v: String = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'g2_attribution_confirmed'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "1", "G2 认可计数 +1");
        // 错题状态推进为 analyzed
        assert_eq!(
            MistakeDao::new(&conn).get(f.mistake_id).unwrap().status,
            "analyzed"
        );
    }

    #[test]
    fn confirm_with_edit_creates_user_version() {
        let f = fixture("attr-edit");
        f.engine
            .confirm(AttributionConfirm {
                attribution_id: f.local_id,
                edited_label: Some("用户修正标签".into()),
                edited_detail: None,
                edited_kg_node_id: None,
            })
            .unwrap();
        let conn = f.db.conn().unwrap();
        let dao = AttributionDao::new(&conn);
        let active = dao.active_for_mistake(f.mistake_id).unwrap().unwrap();
        assert_eq!(active.origin, "user");
        assert_eq!(active.cause_label, "用户修正标签");
        // 原版本降级保留，G2 修改计数 +1
        let original = dao.get(f.local_id).unwrap();
        assert!(!original.is_active);
        assert_eq!(original.superseded_by, Some(active.id));
        let v: String = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'g2_attribution_edited'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "1");
    }

    #[test]
    fn attribution_mode_parse() {
        assert_eq!(
            AttributionMode::parse("auto", true).unwrap(),
            AttributionMode::Cloud
        );
        assert_eq!(
            AttributionMode::parse("auto", false).unwrap(),
            AttributionMode::Local
        );
        assert!(AttributionMode::parse("weird", true).is_err());
    }
}
