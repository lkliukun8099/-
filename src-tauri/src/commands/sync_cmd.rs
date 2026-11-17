//! 端到端加密同步命令（F-21，P1 桩）。

use tauri::State;

use crate::error::AppError;
use crate::{sync, AppState};

/// 推送本地变更（P1 预留）。
#[tauri::command(rename_all = "camelCase")]
pub fn sync_push(_state: State<'_, AppState>) -> Result<(), AppError> {
    sync::sync_push()
}

/// 拉取远端变更（P1 预留）。
#[tauri::command(rename_all = "camelCase")]
pub fn sync_pull(_state: State<'_, AppState>) -> Result<(), AppError> {
    sync::sync_pull()
}
