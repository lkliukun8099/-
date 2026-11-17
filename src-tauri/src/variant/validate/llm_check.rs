//! 校验闸 3：LLM 交叉自校验（用另一模型独立解题并比对答案，防幻觉）。

use std::sync::Arc;

use super::GateResult;
use crate::ai::adapter::{ChatMessage, ChatRequest};
use crate::ai::Gateway;
use crate::db::dao::variant::VariantDraft;

/// 交叉校验系统提示词。
const SELF_CHECK_SYSTEM: &str = r#"你是一位严格的中学数学验题老师。给定一道题目及其命题人给出的答案，
请你独立求解并判断答案是否正确。只输出 JSON：
{"pass":true|false,"reason":"判断依据（50字内）"}"#;

/// 执行 LLM 交叉校验。
///
/// 校验失败（网络/模型不可用）时返回 skipped=true 的通过结果——
/// 不因为校验链路故障误杀题目，由 rules+symbolic 两道闸保底。
pub async fn check(gateway: &Arc<Gateway>, draft: &VariantDraft) -> GateResult {
    let prompt = format!(
        "【题目】\n{}\n{}\n\n【命题人答案】\n{}\n\n【命题人解析】\n{}",
        draft.content,
        draft.content_latex.clone().unwrap_or_default(),
        draft.answer,
        draft.solution.clone().unwrap_or_else(|| "（无）".into())
    );
    let req = ChatRequest {
        model: String::new(),
        messages: vec![
            ChatMessage::system(SELF_CHECK_SYSTEM),
            ChatMessage::user(prompt),
        ],
        temperature: Some(0.0),
        max_tokens: Some(400),
    };

    match gateway.chat_once("llm_check", req).await {
        Ok(resp) => match parse_verdict(&resp.content) {
            Some((pass, reason)) => GateResult {
                gate: "llm_check",
                passed: pass,
                skipped: false,
                detail: reason,
            },
            None => GateResult {
                gate: "llm_check",
                passed: true,
                skipped: true,
                detail: "LLM 校验输出无法解析，按跳过处理".into(),
            },
        },
        Err(e) => {
            tracing::warn!("LLM 交叉校验不可用（降级跳过）: {e}");
            GateResult {
                gate: "llm_check",
                passed: true,
                skipped: true,
                detail: format!("校验模型不可用，降级跳过: {}", e.message),
            }
        }
    }
}

/// 解析校验结论 JSON（容忍围栏与杂讯）。
fn parse_verdict(raw: &str) -> Option<(bool, String)> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&raw[start..=end]).ok()?;
    let pass = value.get("pass")?.as_bool()?;
    let reason = value
        .get("reason")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();
    Some((pass, reason))
}
