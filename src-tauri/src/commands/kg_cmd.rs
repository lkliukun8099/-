//! 知识图谱命令（F-07）。

use tauri::State;

use crate::db::dao::kg::{KgNode, KgTree};
use crate::error::AppError;
use crate::{kg, AppState};

/// 学科三级图谱树。
#[tauri::command(rename_all = "camelCase")]
pub fn kg_tree(state: State<'_, AppState>, subject_id: i64) -> Result<KgTree, AppError> {
    kg::kg_tree(&state.db, subject_id)
}

/// 图谱搜索。
#[tauri::command(rename_all = "camelCase")]
pub fn kg_search(
    state: State<'_, AppState>,
    subject_id: i64,
    keyword: String,
) -> Result<Vec<KgNode>, AppError> {
    kg::kg_search(&state.db, subject_id, &keyword)
}
