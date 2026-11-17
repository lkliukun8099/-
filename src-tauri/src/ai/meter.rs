//! 成本计量：token 估算 + 费用计算 + call_log 落库（AC-7：误差 ≤1%）。
//!
//! 隐私（架构 §9-7）：请求内容只存 SHA-256 哈希与脱敏摘要，不存原文。

use sha2::{Digest, Sha256};

use crate::ai::adapter::Price;
use crate::db::dao::log::LogDao;
use crate::db::DbPool;
use crate::error::AppError;

/// 计量器。
pub struct Meter {
    db: DbPool,
}

impl Meter {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    /// 计算单次调用成本（分）。
    pub fn cost_cny(price: Option<Price>, tokens_in: u32, tokens_out: u32) -> f64 {
        match price {
            Some(p) => {
                (tokens_in as f64 / 1000.0) * p.price_in
                    + (tokens_out as f64 / 1000.0) * p.price_out
            }
            None => 0.0,
        }
    }

    /// 请求内容哈希（隐私：不落原文）。
    pub fn request_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        crate::db::hex_encode(&hasher.finalize())
    }

    /// 记录一条调用日志。
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        provider: &str,
        model: &str,
        task_type: &str,
        tokens_in: u32,
        tokens_out: u32,
        cost_cny: f64,
        latency_ms: Option<i64>,
        status: &str,
        error_code: Option<&str>,
        request_hash: Option<&str>,
    ) {
        let result = (|| -> Result<(), AppError> {
            let conn = self.db.conn()?;
            LogDao::new(&conn).insert_call_log(
                provider,
                model,
                task_type,
                tokens_in as i64,
                tokens_out as i64,
                cost_cny,
                latency_ms,
                status,
                error_code,
                request_hash,
            )
        })();
        if let Err(e) = result {
            // 计量失败不影响主流程，仅告警
            tracing::warn!("调用日志写入失败: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_calculation() {
        // 1.0 分/千token 输入 + 2.0 分/千token 输出，各 1000 token → 3.0 分
        let price = Some(Price {
            price_in: 1.0,
            price_out: 2.0,
        });
        let cost = Meter::cost_cny(price, 1000, 1000);
        assert!((cost - 3.0).abs() < 1e-9);
    }

    #[test]
    fn cost_fractional_tokens() {
        let price = Some(Price {
            price_in: 1.0,
            price_out: 2.0,
        });
        // 500 in + 250 out → 0.5 + 0.5 = 1.0 分
        let cost = Meter::cost_cny(price, 500, 250);
        assert!((cost - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cost_zero_when_no_price() {
        assert_eq!(Meter::cost_cny(None, 10000, 10000), 0.0);
    }

    #[test]
    fn request_hash_is_sha256_hex() {
        // sha256("hello") 公认值
        assert_eq!(
            Meter::request_hash("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        // 64 位小写 hex
        let h = Meter::request_hash("任意请求内容");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn request_hash_distinct_inputs_distinct_hashes() {
        assert_ne!(Meter::request_hash("a"), Meter::request_hash("b"));
    }

    #[test]
    fn record_writes_call_log_without_plaintext() {
        // 计量落库：request_hash 存哈希，内容原文不落库（隐私 AC-9）
        let pool = crate::db::DbPool::test_pool("meter").unwrap();
        let meter = Meter::new(pool.clone());
        let secret_content = "学生张三的错题内容";
        let hash = Meter::request_hash(secret_content);
        meter.record("deepseek", "deepseek-chat", "ocr_attribution", 10, 20, 0.3,
                     Some(123), "ok", None, Some(&hash));
        let conn = pool.conn().unwrap();
        let stored_hash: String = conn
            .query_row("SELECT request_hash FROM call_log LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(stored_hash, hash);
        assert!(!stored_hash.contains("张三"), "日志不得包含明文 PII");
        let (tin, tout, status): (i64, i64, String) = conn
            .query_row("SELECT tokens_in, tokens_out, status FROM call_log LIMIT 1", [],
                       |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap();
        assert_eq!((tin, tout, status.as_str()), (10, 20, "ok"));
    }
}
