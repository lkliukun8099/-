//! 调用日志 DAO：成本计量 + OCR 任务留痕（脱敏，仅存哈希与摘要）。

use rusqlite::{params, Connection, Row};
use serde::Serialize;

use crate::error::AppError;

/// AI 调用日志行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallLog {
    pub id: i64,
    pub provider: String,
    pub model: String,
    pub task_type: String,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_cny: f64,
    pub latency_ms: Option<i64>,
    pub status: String,
    pub error_code: Option<String>,
    pub request_hash: Option<String>,
    pub created_at: String,
}

/// 成本报表（F-19 面板）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostReport {
    pub total_cost_cny: f64,
    pub total_calls: i64,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub by_provider: Vec<CostByProvider>,
    pub by_day: Vec<CostByDay>,
    pub recent_logs: Vec<CallLog>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostByProvider {
    pub provider: String,
    pub cost_cny: f64,
    pub calls: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CostByDay {
    pub date: String,
    pub cost_cny: f64,
    pub calls: i64,
}

pub struct LogDao<'a> {
    conn: &'a Connection,
}

impl<'a> LogDao<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_call_log(row: &Row) -> rusqlite::Result<CallLog> {
        Ok(CallLog {
            id: row.get("id")?,
            provider: row.get("provider")?,
            model: row.get("model")?,
            task_type: row.get("task_type")?,
            tokens_in: row.get("tokens_in")?,
            tokens_out: row.get("tokens_out")?,
            cost_cny: row.get("cost_cny")?,
            latency_ms: row.get("latency_ms")?,
            status: row.get("status")?,
            error_code: row.get("error_code")?,
            request_hash: row.get("request_hash")?,
            created_at: row.get("created_at")?,
        })
    }

    /// 写入一条调用日志（Meter 的唯一出口）。
    #[allow(clippy::too_many_arguments)]
    pub fn insert_call_log(
        &self,
        provider: &str,
        model: &str,
        task_type: &str,
        tokens_in: i64,
        tokens_out: i64,
        cost_cny: f64,
        latency_ms: Option<i64>,
        status: &str,
        error_code: Option<&str>,
        request_hash: Option<&str>,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT INTO call_log(provider, model, task_type, tokens_in, tokens_out,
               cost_cny, latency_ms, status, error_code, request_hash)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                provider, model, task_type, tokens_in, tokens_out, cost_cny, latency_ms,
                status, error_code, request_hash,
            ],
        )?;
        Ok(())
    }

    /// 成本报表（range_days 天内）。
    pub fn cost_report(&self, range_days: i64) -> Result<CostReport, AppError> {
        let since = format!("-{range_days} days");

        let (total_cost, total_calls, total_in, total_out): (f64, i64, i64, i64) =
            self.conn.query_row(
                "SELECT COALESCE(SUM(cost_cny),0), COUNT(*),
                        COALESCE(SUM(tokens_in),0), COALESCE(SUM(tokens_out),0)
                 FROM call_log WHERE created_at >= datetime('now', ?1)",
                params![since],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;

        let mut stmt = self.conn.prepare(
            "SELECT provider, COALESCE(SUM(cost_cny),0), COUNT(*) FROM call_log
             WHERE created_at >= datetime('now', ?1)
             GROUP BY provider ORDER BY 2 DESC",
        )?;
        let by_provider = stmt
            .query_map(params![since], |r| {
                Ok(CostByProvider {
                    provider: r.get(0)?,
                    cost_cny: r.get(1)?,
                    calls: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut stmt = self.conn.prepare(
            "SELECT date(created_at), COALESCE(SUM(cost_cny),0), COUNT(*) FROM call_log
             WHERE created_at >= datetime('now', ?1)
             GROUP BY date(created_at) ORDER BY 1",
        )?;
        let by_day = stmt
            .query_map(params![since], |r| {
                Ok(CostByDay {
                    date: r.get(0)?,
                    cost_cny: r.get(1)?,
                    calls: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut stmt = self.conn.prepare(
            "SELECT * FROM call_log ORDER BY id DESC LIMIT 100",
        )?;
        let recent_logs = stmt
            .query_map([], Self::row_to_call_log)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(CostReport {
            total_cost_cny: total_cost,
            total_calls,
            total_tokens_in: total_in,
            total_tokens_out: total_out,
            by_provider,
            by_day,
            recent_logs,
        })
    }

    /// 清空调用日志（隐私：一键清除，N-03）。
    pub fn clear_call_logs(&self) -> Result<usize, AppError> {
        let n = self.conn.execute("DELETE FROM call_log", [])?;
        Ok(n)
    }

    /// 记录 OCR 任务（双引擎留痕）。
    pub fn insert_ocr_job(
        &self,
        mistake_id: i64,
        engine: &str,
        status: &str,
        result_text: Option<&str>,
        result_latex: Option<&str>,
        confidence: Option<f64>,
        latency_ms: Option<i64>,
    ) -> Result<i64, AppError> {
        self.conn.execute(
            "INSERT INTO ocr_job(mistake_id, engine, status, result_text, result_latex,
               confidence, latency_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![mistake_id, engine, status, result_text, result_latex, confidence, latency_ms],
        )?;
        Ok(self.conn.last_insert_rowid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbPool;

    #[test]
    fn cost_report_aggregates() {
        let conn = DbPool::memory_conn().unwrap();
        let dao = LogDao::new(&conn);
        dao.insert_call_log("deepseek", "deepseek-chat", "ocr_attribution", 100, 200, 0.5,
                            Some(800), "ok", None, Some("hash-a")).unwrap();
        dao.insert_call_log("deepseek", "deepseek-chat", "variant_gen", 100, 100, 0.3,
                            Some(88), "ok", None, Some("hash-b")).unwrap();
        dao.insert_call_log("glm", "glm-4-flash", "llm_check", 0, 0, 0.0,
                            None, "circuit_open", Some("CIRCUIT_OPEN"), Some("hash-c")).unwrap();

        let report = dao.cost_report(30).unwrap();
        assert_eq!(report.total_calls, 3);
        assert!((report.total_cost_cny - 0.8).abs() < 1e-9);
        assert_eq!(report.total_tokens_in, 200);
        assert_eq!(report.total_tokens_out, 300);
        assert_eq!(report.by_provider[0].provider, "deepseek", "按成本降序");
        assert!((report.by_provider[0].cost_cny - 0.8).abs() < 1e-9);
        assert_eq!(report.by_provider[1].provider, "glm");
        assert_eq!(report.recent_logs.len(), 3);
        // 脱敏断言：库里只有哈希，无明文提示词
        assert!(report.recent_logs.iter().all(|l| l
            .request_hash
            .as_deref()
            .map(|h| h.len() <= 64)
            .unwrap_or(true)));
    }

    #[test]
    fn cost_report_empty() {
        let conn = DbPool::memory_conn().unwrap();
        let report = LogDao::new(&conn).cost_report(30).unwrap();
        assert_eq!(report.total_calls, 0);
        assert_eq!(report.total_cost_cny, 0.0);
    }

    #[test]
    fn clear_call_logs() {
        let conn = DbPool::memory_conn().unwrap();
        let dao = LogDao::new(&conn);
        dao.insert_call_log("deepseek", "m", "t", 1, 1, 0.1, None, "ok", None, None)
            .unwrap();
        assert_eq!(dao.clear_call_logs().unwrap(), 1);
        assert_eq!(dao.cost_report(30).unwrap().total_calls, 0);
    }

    #[test]
    fn ocr_job_trace() {
        let conn = DbPool::memory_conn().unwrap();
        // ocr_job 外键引用 mistake，先建一题
        let mid = crate::db::dao::mistake::MistakeDao::new(&conn)
            .insert(1, "images/a.png")
            .unwrap();
        let dao = LogDao::new(&conn);
        let id = dao
            .insert_ocr_job(mid, "tesseract", "done", Some("题干"), None, Some(0.88), Some(1200))
            .unwrap();
        assert!(id > 0);
        let (engine, status): (String, String) = conn
            .query_row("SELECT engine, status FROM ocr_job WHERE id = ?1", [id],
                       |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((engine.as_str(), status.as_str()), ("tesseract", "done"));
    }
}
