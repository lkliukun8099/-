//! 复习计划 DAO：类艾宾浩斯排程的持久化。

use rusqlite::{params, Connection, Row};
use serde::Serialize;

use crate::error::AppError;

/// 复习计划行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPlan {
    pub id: i64,
    pub profile_id: i64,
    pub target_type: String,
    pub target_id: i64,
    pub stage: i64,
    pub next_due_at: String,
    pub status: String,
    pub completed_at: Option<String>,
    pub created_at: String,
}

/// 今日待复习项（附目标摘要，供 UI 直接展示）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub plan: ReviewPlan,
    /// 题干摘要（错题取 ocr_text 截断，变式取 content 截断）
    pub title: String,
    pub subject_name: Option<String>,
}

pub struct ReviewDao<'a> {
    conn: &'a Connection,
}

impl<'a> ReviewDao<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_plan(row: &Row) -> rusqlite::Result<ReviewPlan> {
        Ok(ReviewPlan {
            id: row.get("id")?,
            profile_id: row.get("profile_id")?,
            target_type: row.get("target_type")?,
            target_id: row.get("target_id")?,
            stage: row.get("stage")?,
            next_due_at: row.get("next_due_at")?,
            status: row.get("status")?,
            completed_at: row.get("completed_at")?,
            created_at: row.get("created_at")?,
        })
    }

    /// 读取排程间隔配置（JSON 天数数组，默认 [1,3,7,15,30]）。
    pub fn intervals(&self, profile_id: i64) -> Result<Vec<i64>, AppError> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT intervals FROM review_config WHERE profile_id = ?1",
                params![profile_id],
                |r| r.get(0),
            )
            .ok();
        match raw {
            Some(s) => Ok(serde_json::from_str(&s).unwrap_or_else(|_| vec![1, 3, 7, 15, 30])),
            None => Ok(vec![1, 3, 7, 15, 30]),
        }
    }

    /// 更新排程间隔配置。
    pub fn set_intervals(&self, profile_id: i64, intervals: &[i64]) -> Result<(), AppError> {
        let json = serde_json::to_string(intervals)?;
        self.conn.execute(
            "INSERT INTO review_config(profile_id, intervals) VALUES (?1, ?2)
             ON CONFLICT(profile_id) DO UPDATE SET intervals = excluded.intervals",
            params![profile_id, json],
        )?;
        Ok(())
    }

    /// 新建复习计划（target 已有 pending 计划时不重复）。
    pub fn upsert_plan(
        &self,
        profile_id: i64,
        target_type: &str,
        target_id: i64,
        stage: i64,
        next_due_at: &str,
    ) -> Result<i64, AppError> {
        let existing: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM review_plan
                 WHERE target_type = ?1 AND target_id = ?2 AND status = 'pending'",
                params![target_type, target_id],
                |r| r.get(0),
            )
            .ok();
        if let Some(id) = existing {
            // 重置到新的排程起点
            self.conn.execute(
                "UPDATE review_plan SET stage = ?2, next_due_at = ?3 WHERE id = ?1",
                params![id, stage, next_due_at],
            )?;
            return Ok(id);
        }
        self.conn.execute(
            "INSERT INTO review_plan(profile_id, target_type, target_id, stage, next_due_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![profile_id, target_type, target_id, stage, next_due_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 按计划 ID 取。
    pub fn get(&self, id: i64) -> Result<ReviewPlan, AppError> {
        self.conn
            .query_row(
                "SELECT * FROM review_plan WHERE id = ?1",
                params![id],
                Self::row_to_plan,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::not_found(format!("复习计划 #{id} 不存在"))
                }
                other => other.into(),
            })
    }

    /// 推进计划状态（complete 后由 service 决定是否插下一段）。
    pub fn mark_done(&self, id: i64) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE review_plan SET status = 'done', completed_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// 今日到期队列（pending 且 next_due_at ≤ 今天末；时间口径统一 UTC）。
    pub fn due_today(&self, profile_id: i64) -> Result<Vec<ReviewPlan>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM review_plan
             WHERE profile_id = ?1 AND status = 'pending'
               AND next_due_at <= datetime('now', 'start of day', '+1 day')
             ORDER BY next_due_at",
        )?;
        let rows = stmt
            .query_map(params![profile_id], Self::row_to_plan)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 未来 N 天的计划（复习日历用）。
    pub fn upcoming(&self, profile_id: i64, days: i64) -> Result<Vec<ReviewPlan>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM review_plan
             WHERE profile_id = ?1 AND status = 'pending'
               AND next_due_at <= datetime('now', ?2)
             ORDER BY next_due_at",
        )?;
        let rows = stmt
            .query_map(params![profile_id, format!("+{days} days")], Self::row_to_plan)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 目标摘要（错题取 OCR 文本，变式取题干），截断 80 字。
    pub fn target_title(&self, target_type: &str, target_id: i64) -> Result<String, AppError> {
        let text: Option<String> = match target_type {
            "mistake" => self
                .conn
                .query_row(
                    "SELECT COALESCE(ocr_text, '(未识别)') FROM mistake WHERE id = ?1",
                    params![target_id],
                    |r| r.get(0),
                )
                .ok(),
            "variant" => self
                .conn
                .query_row(
                    "SELECT content FROM variant_question WHERE id = ?1",
                    params![target_id],
                    |r| r.get(0),
                )
                .ok(),
            _ => None,
        };
        let text = text.unwrap_or_else(|| "(目标已删除)".to_string());
        Ok(text.chars().take(80).collect())
    }

    /// 错题的全部复习计划（详情页展示）。
    pub fn plans_for_target(
        &self,
        target_type: &str,
        target_id: i64,
    ) -> Result<Vec<ReviewPlan>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM review_plan WHERE target_type = ?1 AND target_id = ?2
             ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map(params![target_type, target_id], Self::row_to_plan)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 过期计划清理（超过 7 天未完成标记 expired，后台任务用）。
    pub fn expire_overdue(&self, days: i64) -> Result<usize, AppError> {
        let n = self.conn.execute(
            "UPDATE review_plan SET status = 'expired'
             WHERE status = 'pending' AND next_due_at <= datetime('now', ?1)",
            params![format!("-{days} days")],
        )?;
        Ok(n)
    }
}
