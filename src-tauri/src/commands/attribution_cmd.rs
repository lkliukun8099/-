//! 归因与冲突合并命令（F-06/F-16）。

use tauri::State;

use crate::attribution::{AttributionEngine, AttributionMode};
use crate::db::dao::attribution::{Attribution, AttributionConfirm, AttributionDao, Conflict};
use crate::error::AppError;
use crate::kg;
use crate::AppState;

/// 运行归因（mode: local / cloud / auto）。
#[tauri::command(rename_all = "camelCase")]
pub async fn attribution_run(
    state: State<'_, AppState>,
    mistake_id: i64,
    mode: String,
) -> Result<Attribution, AppError> {
    let mode = AttributionMode::parse(&mode, state.is_online())?;
    let engine = AttributionEngine::new(state.db.clone(), state.gateway.clone());
    engine.analyze(mistake_id, mode).await
}

/// 用户认可/修改归因（G2 指标埋点）。
#[tauri::command(rename_all = "camelCase")]
pub fn attribution_confirm(
    state: State<'_, AppState>,
    input: AttributionConfirm,
) -> Result<(), AppError> {
    let engine = AttributionEngine::new(state.db.clone(), state.gateway.clone());
    engine.confirm(input)
}

/// 冲突列表（默认 pending）。
#[tauri::command(rename_all = "camelCase")]
pub fn conflict_list(
    state: State<'_, AppState>,
    status: Option<String>,
) -> Result<Vec<Conflict>, AppError> {
    let conn = state.db.conn()?;
    let dao = AttributionDao::new(&conn);
    let mut conflicts = dao.list_conflicts(status.as_deref().or(Some("pending")))?;
    // 回填图谱路径供 Diff 展示
    for c in &mut conflicts {
        if let Some(nid) = c.local_attr.kg_node_id {
            c.local_attr.kg_path = kg::node_path(&conn, nid);
        }
        if let Some(nid) = c.cloud_attr.kg_node_id {
            c.cloud_attr.kg_path = kg::node_path(&conn, nid);
        }
    }
    Ok(conflicts)
}

/// 裁决冲突（choice: local / cloud / merged；merged 需提供 merged_label）。
#[tauri::command(rename_all = "camelCase")]
pub fn conflict_resolve(
    state: State<'_, AppState>,
    conflict_id: i64,
    choice: String,
    merged_label: Option<String>,
    merged_detail: Option<String>,
) -> Result<(), AppError> {
    let engine = AttributionEngine::new(state.db.clone(), state.gateway.clone());
    engine.resolve_conflict(conflict_id, &choice, merged_label, merged_detail)
}
