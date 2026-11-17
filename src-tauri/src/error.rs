//! 统一错误类型与错误码。
//!
//! 错误码段分配（架构文档 §9-1）：
//! - `1xxx` 通用/参数
//! - `2xxx` 数据库
//! - `3xxx` OCR
//! - `4xxx` AI 接入层（4001 全链路熔断不可用、4002 预算超限）
//! - `5xxx` 归因/校验
//! - `6xxx` 同步

use serde::Serialize;

/// 统一应用错误，序列化为 `{code, message, detail?}` 返回前端。
#[derive(Debug, Clone, Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("[{code}] {message}")]
pub struct AppError {
    pub code: u32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AppError {
    /// 构造任意错误码的错误。
    pub fn new(code: u32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }

    /// 附带细节信息。
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    // ---- 1xxx 通用/参数 ----

    /// 1001 参数非法
    pub fn invalid_param(message: impl Into<String>) -> Self {
        Self::new(1001, message)
    }

    /// 1002 资源不存在
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(1002, message)
    }

    /// 1003 内部错误
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(1003, message)
    }

    // ---- 2xxx 数据库 ----

    /// 2001 数据库错误
    pub fn db(message: impl Into<String>) -> Self {
        Self::new(2001, message)
    }

    /// 2002 数据库迁移失败
    pub fn migrate(message: impl Into<String>) -> Self {
        Self::new(2002, message)
    }

    /// 2003 备份/恢复失败
    pub fn backup(message: impl Into<String>) -> Self {
        Self::new(2003, message)
    }

    /// 2004 乐观锁冲突（版本不一致）
    pub fn version_conflict(message: impl Into<String>) -> Self {
        Self::new(2004, message)
    }

    // ---- 3xxx OCR ----

    /// 3001 OCR 引擎调用失败
    pub fn ocr(message: impl Into<String>) -> Self {
        Self::new(3001, message)
    }

    /// 3002 所有 OCR 引擎均不可用
    pub fn ocr_unavailable(message: impl Into<String>) -> Self {
        Self::new(3002, message)
    }

    // ---- 4xxx AI 接入层 ----

    /// 4001 全链路熔断/不可用
    pub fn ai_unavailable(message: impl Into<String>) -> Self {
        Self::new(4001, message)
    }

    /// 4002 预算超限
    pub fn budget_exceeded(message: impl Into<String>) -> Self {
        Self::new(4002, message)
    }

    /// 4003 AI 调用失败（上游错误/超时）
    pub fn ai_call(message: impl Into<String>) -> Self {
        Self::new(4003, message)
    }

    /// 4004 密钥管理失败（keyring 读写）
    pub fn keychain(message: impl Into<String>) -> Self {
        Self::new(4004, message)
    }

    // ---- 5xxx 归因/校验 ----

    /// 5001 归因失败
    pub fn attribution(message: impl Into<String>) -> Self {
        Self::new(5001, message)
    }

    /// 5002 变式生成/校验失败
    pub fn validate(message: impl Into<String>) -> Self {
        Self::new(5002, message)
    }

    // ---- 6xxx 同步 ----

    /// 6001 同步未实现（P1 桩）
    pub fn sync_unimplemented() -> Self {
        Self::new(6001, "端到端加密同步为 P1 预留功能，当前版本未实现")
    }
}

// ---- 常见错误自动转换 ----

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::db(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::internal(format!("JSON 序列化错误: {e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::internal(format!("IO 错误: {e}"))
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            AppError::ai_call(format!("请求超时: {e}"))
        } else {
            AppError::ai_call(e.to_string())
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::internal(e.to_string())
    }
}
