//! 掌握度 DAO：EMA 分数 + 历史快照 + 作答记录 + 诊断报告查询。

use rusqlite::{params, Connection, Row};
use serde::Serialize;

use crate::error::AppError;

/// 掌握度行（每档案×知识节点）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mastery {
    pub id: i64,
    pub profile_id: i64,
    pub kg_node_id: i64,
    pub score: f64,
    pub attempts: i64,
    pub correct: i64,
    pub last_assessed_at: Option<String>,
}

/// 热力图单元（F-13）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatCell {
    pub kg_node_id: i64,
    pub node_name: String,
    pub node_path: Vec<String>,
    pub score: f64,
    pub attempts: i64,
}

/// 掌握度曲线点。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurvePoint {
    pub date: String,
    pub avg_score: f64,
}

/// 错因分布项。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CauseCount {
    pub cause_label: String,
    pub count: i64,
}

/// 诊断报告聚合 DTO（mastery_report 返回）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDto {
    pub heatmap: Vec<HeatCell>,
    pub curve: Vec<CurvePoint>,
    pub cause_distribution: Vec<CauseCount>,
    pub total_mistakes: i64,
    pub reviewed_today: i64,
}

pub struct MasteryDao<'a> {
    conn: &'a Connection,
}

impl<'a> MasteryDao<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_mastery(row: &Row) -> rusqlite::Result<Mastery> {
        Ok(Mastery {
            id: row.get("id")?,
            profile_id: row.get("profile_id")?,
            kg_node_id: row.get("kg_node_id")?,
            score: row.get("score")?,
            attempts: row.get("attempts")?,
            correct: row.get("correct")?,
            last_assessed_at: row.get("last_assessed_at")?,
        })
    }

    /// 取单节点掌握度。
    pub fn get(&self, profile_id: i64, kg_node_id: i64) -> Result<Option<Mastery>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM mastery WHERE profile_id = ?1 AND kg_node_id = ?2")?;
        let mut rows = stmt.query_map(params![profile_id, kg_node_id], Self::row_to_mastery)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 档案下全部掌握度。
    pub fn list_for_profile(&self, profile_id: i64) -> Result<Vec<Mastery>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM mastery WHERE profile_id = ?1 ORDER BY score")?;
        let rows = stmt
            .query_map(params![profile_id], Self::row_to_mastery)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// EMA 更新（存在则改，不存在则建）。
    pub fn upsert_score(
        &self,
        profile_id: i64,
        kg_node_id: i64,
        new_score: f64,
        is_correct: bool,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT INTO mastery(profile_id, kg_node_id, score, attempts, correct, last_assessed_at)
             VALUES (?1, ?2, ?3, 1, ?4, datetime('now'))
             ON CONFLICT(profile_id, kg_node_id) DO UPDATE SET
               score = ?3,
               attempts = attempts + 1,
               correct = correct + ?4,
               last_assessed_at = datetime('now')",
            params![profile_id, kg_node_id, new_score, is_correct as i64],
        )?;
        Ok(())
    }

    /// 记录历史快照（掌握度曲线数据源）。
    pub fn record_history(
        &self,
        profile_id: i64,
        kg_node_id: i64,
        score: f64,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT INTO mastery_history(profile_id, kg_node_id, score) VALUES (?1,?2,?3)",
            params![profile_id, kg_node_id, score],
        )?;
        Ok(())
    }

    /// 插入作答记录。
    pub fn insert_attempt(
        &self,
        profile_id: i64,
        target_type: &str,
        target_id: i64,
        answer: Option<&str>,
        is_correct: bool,
        duration_ms: Option<i64>,
    ) -> Result<i64, AppError> {
        self.conn.execute(
            "INSERT INTO attempt(profile_id, target_type, target_id, answer, is_correct, duration_ms)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![profile_id, target_type, target_id, answer, is_correct as i64, duration_ms],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    // ---- 报告查询（F-13）----

    /// 薄弱热力图：掌握度最低的 N 个节点。
    pub fn heatmap(&self, profile_id: i64, limit: i64) -> Result<Vec<HeatCell>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT m.kg_node_id, k.name, m.score, m.attempts
             FROM mastery m JOIN kg_node k ON k.id = m.kg_node_id
             WHERE m.profile_id = ?1
             ORDER BY m.score ASC, m.attempts DESC LIMIT ?2",
        )?;
        let cells = stmt
            .query_map(params![profile_id, limit], |r| {
                Ok(HeatCell {
                    kg_node_id: r.get(0)?,
                    node_name: r.get(1)?,
                    node_path: Vec::new(), // service 层回填
                    score: r.get(2)?,
                    attempts: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(cells)
    }

    /// 掌握度曲线：按天聚合平均分（range_days 内）。
    pub fn curve(&self, profile_id: i64, range_days: i64) -> Result<Vec<CurvePoint>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT date(created_at) AS d, AVG(score) AS avg_score
             FROM mastery_history
             WHERE profile_id = ?1 AND created_at >= datetime('now', ?2)
             GROUP BY d ORDER BY d",
        )?;
        let rows = stmt
            .query_map(params![profile_id, format!("-{range_days} days")], |r| {
                Ok(CurvePoint {
                    date: r.get(0)?,
                    avg_score: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 错因分布（当前生效归因按标签计数）。
    pub fn cause_distribution(&self, profile_id: i64) -> Result<Vec<CauseCount>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT a.cause_label, COUNT(*) AS cnt
             FROM attribution a
             JOIN mistake m ON m.id = a.mistake_id
             WHERE m.profile_id = ?1 AND m.deleted = 0 AND a.is_active = 1
             GROUP BY a.cause_label ORDER BY cnt DESC LIMIT 20",
        )?;
        let rows = stmt
            .query_map(params![profile_id], |r| {
                Ok(CauseCount {
                    cause_label: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 今日已完成复习数（报告卡用）。
    pub fn reviewed_today(&self, profile_id: i64) -> Result<i64, AppError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM review_plan
             WHERE profile_id = ?1 AND status = 'done' AND date(completed_at) = date('now')",
            params![profile_id],
            |r| r.get(0),
        )?;
        Ok(count)
    }
}
