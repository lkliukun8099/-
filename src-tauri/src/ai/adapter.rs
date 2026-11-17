//! 模型适配器抽象：OpenAI 兼容协议为基线（架构 §4.2）。

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 对话消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
}

/// 统一聊天请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    /// 目标模型（路由层填充，前端传空即可）
    #[serde(default)]
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// 非流式响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub content: String,
    pub model: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    /// 实际出单的 provider.model（降级时与请求不同）
    pub served_by: String,
}

/// token 用量。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub tokens_in: u32,
    pub tokens_out: u32,
}

/// 流式分块（Channel 桥接协议，与 TS `StreamChunk` 镜像，架构 §4.4）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunk {
    pub delta: String,
    pub done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 降级标记：非主力模型出单时带原目标名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded: Option<String>,
}

impl StreamChunk {
    pub fn delta(text: impl Into<String>) -> Self {
        Self {
            delta: text.into(),
            done: false,
            usage: None,
            degraded: None,
        }
    }
    pub fn done(usage: Option<Usage>, degraded: Option<String>) -> Self {
        Self {
            delta: String::new(),
            done: true,
            usage,
            degraded,
        }
    }
}

/// 流式响应类型。
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<StreamChunk, AppError>> + Send>>;

/// 价格（分/千 token）。
#[derive(Debug, Clone, Copy)]
pub struct Price {
    pub price_in: f64,
    pub price_out: f64,
}

/// 模型供应商适配器。
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// 适配器标识（provider 名）。
    fn name(&self) -> &str;

    /// 流式聊天。
    async fn chat_stream(&self, req: ChatRequest) -> Result<ChatStream, AppError>;

    /// 一次性聊天。
    async fn chat_once(&self, req: ChatRequest) -> Result<ChatResponse, AppError>;

    /// token 估算（tiktoken cl100k，误差 ≤1%，AC-7）。
    fn estimate_tokens(&self, text: &str) -> u32;

    /// 模型价格（无配置返回 None，成本按 0 计）。
    fn pricing(&self, model: &str) -> Option<Price>;
}
