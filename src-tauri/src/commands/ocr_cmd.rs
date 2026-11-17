//! OCR 命令（F-04）。

use tauri::State;

use crate::error::AppError;
use crate::ocr::{orchestrator_from, OcrPrefer, OcrResult};
use crate::AppState;

/// 运行 OCR 流水线（双引擎编排 + 一致性校验）。
///
/// `prefer`: auto / cloud / offline
#[tauri::command(rename_all = "camelCase")]
pub async fn ocr_run(
    state: State<'_, AppState>,
    mistake_id: i64,
    prefer: Option<String>,
) -> Result<OcrResult, AppError> {
    let prefer = OcrPrefer::parse(prefer.as_deref().unwrap_or("auto"))?;
    let orchestrator = orchestrator_from(&state);
    orchestrator.run(mistake_id, prefer).await
}
