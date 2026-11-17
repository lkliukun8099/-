//! 归因 DAO：多版本归因 + 冲突合并（AC-8，双版本永久保留）。

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 归因记录（多版本：local 初判 / cloud 深判 / user 人工）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attribution {
    pub id: i64,
    pub mistake_id: i64,
    pub origin: String,
    pub engine: String,
    pub cause_label: String,
    pub cause_detail: Option<String>,
    pub kg_node_id: Option<i64>,
    /// 三级节点路径（如 ["函数","二次函数","对称轴与区间关系"]），查询时组装
    pub kg_path: Vec<String>,
    pub confidence: f64,
    pub is_active: bool,
    pub superseded_by: Option<i64>,
    pub created_at: String,
}

/// 新建归因的输入。
#[derive(Debug, Clone)]
pub struct NewAttribution {
    pub mistake_id: i64,
    pub origin: String,
    pub engine: String,
    pub cause_label: String,
    pub cause_detail: Option<String>,
    pub kg_node_id: Option<i64>,
    pub confidence: f64,
    pub is_active: bool,
}

/// 冲突记录（含双版本归因，供合并界面 Diff 展示）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub local_attr: Attribution,
    pub cloud_attr: Attribution,
    pub status: String,
    pub resolved_choice: Option<String>,
    pub created_at: String,
}

/// 用户确认/修改归因的输入（G2 埋点）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionConfirm {
    pub attribution_id: i64,
    pub edited_label: Option<String>,
    pub edited_detail: Option<String>,
    pub edited_kg_node_id: Option<i64>,
}

pub struct AttributionDao<'a> {
    conn: &'a Connection,
}

impl<'a> AttributionDao<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_attribution(row: &Row) -> rusqlite::Result<Attribution> {
        Ok(Attribution {
            id: row.get("id")?,
            mistake_id: row.get("mistake_id")?,
            origin: row.get("origin")?,
            engine: row.get("engine")?,
            cause_label: row.get("cause_label")?,
            cause_detail: row.get("cause_detail")?,
            kg_node_id: row.get("kg_node_id")?,
            kg_path: Vec::new(), // 由 service 层组装
            confidence: row.get("confidence")?,
            is_active: row.get::<_, i64>("is_active")? != 0,
            superseded_by: row.get("superseded_by")?,
            created_at: row.get("created_at")?,
        })
    }

    /// 插入归因版本，返回新 ID。
    pub fn insert(&self, a: &NewAttribution) -> Result<i64, AppError> {
        self.conn.execute(
            "INSERT INTO attribution(mistake_id, origin, engine, cause_label, cause_detail,
               kg_node_id, confidence, is_active)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                a.mistake_id,
                a.origin,
                a.engine,
                a.cause_label,
                a.cause_detail,
                a.kg_node_id,
                a.confidence,
                a.is_active as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 按 ID 取归因。
    pub fn get(&self, id: i64) -> Result<Attribution, AppError> {
        self.conn
            .query_row(
                "SELECT * FROM attribution WHERE id = ?1",
                params![id],
                Self::row_to_attribution,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::not_found(format!("归因 #{id} 不存在"))
                }
                other => other.into(),
            })
    }

    /// 某错题的全部归因版本（新→旧）。
    pub fn list_for_mistake(&self, mistake_id: i64) -> Result<Vec<Attribution>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM attribution WHERE mistake_id = ?1 ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt
            .query_map(params![mistake_id], Self::row_to_attribution)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 当前生效的归因。
    pub fn active_for_mistake(&self, mistake_id: i64) -> Result<Option<Attribution>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM attribution WHERE mistake_id = ?1 AND is_active = 1
             ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![mistake_id], Self::row_to_attribution)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 置某归因失效并记录替代者（双版本保留）。
    pub fn supersede(&self, old_id: i64, new_id: i64) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE attribution SET is_active = 0, superseded_by = ?2 WHERE id = ?1",
            params![old_id, new_id],
        )?;
        Ok(())
    }

    /// 置某归因生效。
    pub fn activate(&self, id: i64) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE attribution SET is_active = 1, superseded_by = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    // ---- 冲突（F-16）----

    /// 登记冲突（同一错题已有 pending 冲突时不重复插入）。
    pub fn insert_conflict(
        &self,
        mistake_id: i64,
        local_attr_id: i64,
        cloud_attr_id: i64,
    ) -> Result<i64, AppError> {
        if let Some(id) = self.find_pending_conflict(mistake_id)? {
            return Ok(id);
        }
        self.conn.execute(
            "INSERT INTO conflict(entity_type, entity_id, local_attr_id, cloud_attr_id)
             VALUES ('attribution', ?1, ?2, ?3)",
            params![mistake_id, local_attr_id, cloud_attr_id],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 查某错题的 pending 冲突。
    pub fn find_pending_conflict(&self, mistake_id: i64) -> Result<Option<i64>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM conflict WHERE entity_id = ?1 AND status = 'pending' LIMIT 1",
        )?;
        let mut rows = stmt.query_map(params![mistake_id], |r| r.get(0))?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 冲突列表（含双版本归因）。
    pub fn list_conflicts(&self, status: Option<&str>) -> Result<Vec<Conflict>, AppError> {
        let sql = match status {
            Some(_) => "SELECT * FROM conflict WHERE status = ?1 ORDER BY created_at DESC",
            None => "SELECT * FROM conflict ORDER BY created_at DESC",
        };
        let mut stmt = self.conn.prepare(sql)?;
        let map_row = |row: &Row| -> rusqlite::Result<(i64, String, i64, i64, i64, String, Option<String>, String)> {
            Ok((
                row.get("id")?,
                row.get("entity_type")?,
                row.get("entity_id")?,
                row.get("local_attr_id")?,
                row.get("cloud_attr_id")?,
                row.get("status")?,
                row.get("resolved_choice")?,
                row.get("created_at")?,
            ))
        };
        let raw: Vec<_> = if let Some(s) = status {
            stmt.query_map(params![s], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            stmt.query_map([], map_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut out = Vec::with_capacity(raw.len());
        for (id, entity_type, entity_id, local_id, cloud_id, cstatus, choice, created) in raw {
            out.push(Conflict {
                id,
                entity_type,
                entity_id,
                local_attr: self.get(local_id)?,
                cloud_attr: self.get(cloud_id)?,
                status: cstatus,
                resolved_choice: choice,
                created_at: created,
            });
        }
        Ok(out)
    }

    /// 裁决冲突：local/cloud 选一边生效，merged 由调用方先落一条 merged 归因再选之。
    pub fn resolve_conflict(&self, conflict_id: i64, choice: &str) -> Result<(), AppError> {
        let affected = self.conn.execute(
            "UPDATE conflict SET status = 'resolved', resolved_choice = ?2,
               resolved_at = datetime('now')
             WHERE id = ?1 AND status = 'pending'",
            params![conflict_id, choice],
        )?;
        if affected == 0 {
            return Err(AppError::not_found(format!(
                "冲突 #{conflict_id} 不存在或已处理"
            )));
        }
        Ok(())
    }

    /// 忽略冲突。
    pub fn ignore_conflict(&self, conflict_id: i64) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE conflict SET status = 'ignored', resolved_at = datetime('now')
             WHERE id = ?1 AND status = 'pending'",
            params![conflict_id],
        )?;
        Ok(())
    }
}
