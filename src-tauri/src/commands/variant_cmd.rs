//! 变式题命令（F-08/09/15）。

use tauri::ipc::Channel;
use tauri::State;

use crate::db::dao::variant::{VariantDao, VariantDraft, VariantQuestion};
use crate::error::AppError;
use crate::variant::{AttemptFeedback, VariantGenerator};
use crate::AppState;

/// 流式生成变式题（生成 + 三闸校验，结果逐道推入 Channel）。
#[tauri::command(rename_all = "camelCase")]
pub async fn variant_generate(
    state: State<'_, AppState>,
    mistake_id: i64,
    count: Option<u32>,
    channel: Channel<VariantDraft>,
) -> Result<(), AppError> {
    if !state.is_online() {
        return Err(AppError::ai_unavailable(
            "变式生成依赖云端模型，当前处于离线模式",
        ));
    }
    let generator = VariantGenerator::new(state.db.clone(), state.gateway.clone());
    generator.generate(mistake_id, count.unwrap_or(2), channel).await
}

/// 今日训练队列：校验通过的变式题（按时间倒序）。
#[tauri::command(rename_all = "camelCase")]
pub fn variant_train_queue(
    state: State<'_, AppState>,
    profile_id: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<VariantQuestion>, AppError> {
    let conn = state.db.conn()?;
    VariantDao::new(&conn).list_trainable(profile_id.unwrap_or(1), limit.unwrap_or(20).clamp(1, 100))
}

/// 变式作答提交（判分 + 掌握度更新 + 复习排程）。
#[tauri::command(rename_all = "camelCase")]
pub fn variant_submit_attempt(
    state: State<'_, AppState>,
    variant_id: i64,
    answer: String,
    duration_ms: Option<i64>,
    profile_id: Option<i64>,
) -> Result<AttemptFeedback, AppError> {
    if answer.trim().is_empty() {
        return Err(AppError::invalid_param("作答内容不能为空"));
    }
    let generator = VariantGenerator::new(state.db.clone(), state.gateway.clone());
    generator.submit_attempt(variant_id, &answer, duration_ms, profile_id.unwrap_or(1))
}
