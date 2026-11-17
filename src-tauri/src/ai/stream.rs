//! SSE 解析：OpenAI 兼容流式响应 → StreamChunk 流（架构 §4.2 流式桥接）。

use async_stream::try_stream;
use futures::StreamExt;

use super::adapter::{ChatStream, StreamChunk, Usage};
use crate::error::AppError;

/// OpenAI 兼容 SSE 分块的 JSON 结构（按需字段）。
#[derive(Debug, serde::Deserialize)]
struct SseChunk {
    choices: Option<Vec<SseChoice>>,
    usage: Option<SseUsage>,
}

#[derive(Debug, serde::Deserialize)]
struct SseChoice {
    delta: Option<SseDelta>,
}

#[derive(Debug, serde::Deserialize)]
struct SseDelta {
    content: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct SseUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

/// 将 reqwest 响应体解析为 StreamChunk 流。
///
/// 协议：`data: {json}\n\n` 行序列，以 `data: [DONE]` 结束。
pub fn openai_sse_to_stream(resp: reqwest::Response) -> ChatStream {
    Box::pin(try_stream! {
        let mut byte_stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut final_usage: Option<Usage> = None;

        while let Some(item) = byte_stream.next().await {
            let bytes = item.map_err(|e| AppError::ai_call(format!("流读取失败: {e}")))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            // 按行切割，保留尾部不完整行
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.drain(..=pos);
                if !line.starts_with("data:") {
                    continue;
                }
                let payload = line.trim_start_matches("data:").trim();
                if payload == "[DONE]" {
                    yield StreamChunk::done(final_usage, None);
                    return;
                }
                match serde_json::from_str::<SseChunk>(payload) {
                    Ok(chunk) => {
                        if let Some(u) = chunk.usage {
                            final_usage = Some(Usage {
                                tokens_in: u.prompt_tokens.unwrap_or(0),
                                tokens_out: u.completion_tokens.unwrap_or(0),
                            });
                        }
                        if let Some(text) = chunk
                            .choices
                            .and_then(|cs| cs.into_iter().next())
                            .and_then(|c| c.delta)
                            .and_then(|d| d.content)
                        {
                            if !text.is_empty() {
                                yield StreamChunk::delta(text);
                            }
                        }
                    }
                    Err(e) => {
                        // 单个分块解析失败不终止流（容错）
                        tracing::debug!("SSE 分块解析失败（已跳过）: {e}");
                    }
                }
            }
        }
        // 流自然结束（未收到 [DONE]）
        yield StreamChunk::done(final_usage, None);
    })
}
