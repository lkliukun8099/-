//! 端到端加密同步（F-21，P1 预留桩）。
//!
//! 设计约定（架构 §2.2 / §3.2）：
//! - 用户自持密钥（AES-256-GCM，见 `crypto` 模块），服务端仅存密文；
//! - 同步载体为 `SyncEnvelope`：实体类型 + 实体 ID + 密文 + 版本；
//! - 首版仅保留接口定义与 `sync_meta` 表，不实现服务端对接。

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 同步信封（P1 实现时的传输单元）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEnvelope {
    /// 实体类型（mistake/attribution/variant/mastery/review_plan）
    pub entity_type: String,
    pub entity_id: i64,
    /// AES-256-GCM 密文包（base64(nonce||ciphertext)，见 crypto 模块）
    pub ciphertext: String,
    /// 实体版本号（冲突合并依据，与 mistake.version 语义一致）
    pub version: i64,
    pub updated_at: String,
}

/// 推送本地变更（P1 桩）。
pub fn sync_push() -> Result<(), AppError> {
    Err(AppError::sync_unimplemented())
}

/// 拉取远端变更（P1 桩）。
pub fn sync_pull() -> Result<(), AppError> {
    Err(AppError::sync_unimplemented())
}
