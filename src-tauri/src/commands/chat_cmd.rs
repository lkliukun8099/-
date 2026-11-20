//! 对话式错题分析命令（聊天入口）：文本建题 → 归因 → 变式生成，一次调用完成。

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::attribution::AttributionEngine;
use crate::db::dao::attribution::Attribution;
use crate::db::dao::mistake::MistakeDao;
use crate::db::dao::variant::VariantDraft;
use crate::error::AppError;
use crate::variant::VariantGenerator;
use crate::AppState;

/// 对话分析结果：错题 ID + 归因 + 变式草稿 + 备注。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAnalyzeResult {
    pub mistake_id: i64,
    pub attribution: Option<Attribution>,
    pub variants: Vec<VariantDraft>,
    pub note: Option<String>,
}

/// 对话式分析：用户直接输入文本错题（可附错误作答）。
///
/// 流程：文本建题（OCR 结果直写）→ 归因（在线 cloud / 离线 local）→ 变式生成（仅在线，最多 3 道）。
/// 归因/变式失败均不阻塞，以 note 告知降级原因。
#[tauri::command(rename_all = "camelCase")]
pub async fn chat_analyze(
    state: State<'_, AppState>,
    question: String,
    my_answer: Option<String>,
) -> Result<ChatAnalyzeResult, AppError> {
    let question = question.trim().to_string();
    if question.len() < 4 {
        return Err(AppError::invalid_param("题目太短，请写完整后重试"));
    }

    // 1. 文本建题：source_image 以 chat:// 占位，OCR 文本直写（跳过图像流水线）
    let mistake_id = {
        let conn = state.db.conn()?;
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, "chat://text")?;
        dao.update_ocr_result(id, &question, None, "manual-text", 1.0)?;
        if let Some(ans) = my_answer.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
            conn.execute(
                "UPDATE mistake SET student_answer = ?1 WHERE id = ?2",
                rusqlite::params![ans, id],
            )?;
        }
        id
    };

    // 2. 归因（auto：在线云端深判 / 离线本地规则；失败不阻塞）
    let online = state.is_online();
    let engine = AttributionEngine::new(state.db.clone(), state.gateway.clone());
    let mode = crate::attribution::AttributionMode::parse("auto", online)?;
    let attribution = match engine.analyze(mistake_id, mode).await {
        Ok(a) => Some(a),
        Err(e) => {
            tracing::warn!("chat 归因失败 #{mistake_id}: {e}");
            None
        }
    };

    // 3. 变式生成（仅在线；用 Channel 收集流式草稿，取前 3 道）
    let mut variants: Vec<VariantDraft> = Vec::new();
    let mut note: Option<String> = None;
    if online {
        let collected = Arc::new(Mutex::new(Vec::<VariantDraft>::new()));
        let sink = collected.clone();
        let channel: Channel<VariantDraft> = Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                if let Ok(d) = serde_json::from_str::<VariantDraft>(&json) {
                    sink.lock().unwrap().push(d);
                }
            }
            Ok(())
        });
        let generator = VariantGenerator::new(state.db.clone(), state.gateway.clone());
        match generator.generate(mistake_id, 3, channel).await {
            Ok(()) => {
                variants = collected.lock().unwrap().clone();
                if variants.is_empty() {
                    note = Some("变式生成未产出题目（可能校验未通过，可重试）".into());
                }
            }
            Err(e) => {
                tracing::warn!("chat 变式生成失败 #{mistake_id}: {e}");
                note = Some(format!("变式生成失败：{e}"));
            }
        }
    } else {
        note = Some("当前离线：已完成本地规则归因；变式出题需联网调用大模型".into());
    }

    Ok(ChatAnalyzeResult {
        mistake_id,
        attribution,
        variants,
        note,
    })
}
