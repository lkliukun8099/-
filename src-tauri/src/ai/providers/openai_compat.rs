//! OpenAI 兼容协议适配器：DeepSeek / GLM / Kimi / 本地端侧模型通用。

use async_trait::async_trait;
use serde_json::json;

use crate::ai::adapter::{
    ChatRequest, ChatResponse, ChatStream, Price, ProviderAdapter, Usage,
};
use crate::ai::stream::openai_sse_to_stream;
use crate::db::dao::model::ModelSpec;
use crate::error::AppError;

/// OpenAI 兼容适配器。
pub struct OpenAiCompatAdapter {
    http: reqwest::Client,
    provider_name: String,
    base_url: String,
    api_key: String,
    models: Vec<ModelSpec>,
}

/// 非流式响应 JSON（按需字段）。
#[derive(Debug, serde::Deserialize)]
struct OnceResp {
    choices: Option<Vec<OnceChoice>>,
    usage: Option<OnceUsage>,
    error: Option<OnceError>,
}

#[derive(Debug, serde::Deserialize)]
struct OnceChoice {
    message: Option<OnceMessage>,
}

#[derive(Debug, serde::Deserialize)]
struct OnceMessage {
    content: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct OnceUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

#[derive(Debug, serde::Deserialize)]
struct OnceError {
    message: Option<String>,
}

impl OpenAiCompatAdapter {
    pub fn new(
        http: reqwest::Client,
        provider_name: String,
        base_url: String,
        api_key: String,
        models: Vec<ModelSpec>,
    ) -> Self {
        Self {
            http,
            provider_name,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            models,
        }
    }

    /// 组装请求体。
    fn build_body(req: &ChatRequest, stream: bool) -> serde_json::Value {
        let mut body = json!({
            "model": req.model,
            "messages": req.messages,
            "stream": stream,
        });
        if let Some(t) = req.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(m) = req.max_tokens {
            body["max_tokens"] = json!(m);
        }
        if stream {
            // 要求流式末块带 usage（OpenAI 兼容扩展，DeepSeek/GLM/Kimi 均支持）
            body["stream_options"] = json!({"include_usage": true});
        }
        body
    }
}

#[async_trait]
impl ProviderAdapter for OpenAiCompatAdapter {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn chat_stream(&self, req: ChatRequest) -> Result<ChatStream, AppError> {
        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&Self::build_body(&req, true))
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::ai_call(format!(
                "{} HTTP {status}: {}",
                self.provider_name,
                body.chars().take(300).collect::<String>()
            )));
        }
        Ok(openai_sse_to_stream(resp))
    }

    async fn chat_once(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&Self::build_body(&req, false))
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(AppError::ai_call(format!(
                "{} HTTP {status}: {}",
                self.provider_name,
                body.chars().take(300).collect::<String>()
            )));
        }
        let parsed: OnceResp = serde_json::from_str(&body)?;
        if let Some(err) = parsed.error {
            return Err(AppError::ai_call(format!(
                "{}: {}",
                self.provider_name,
                err.message.unwrap_or_else(|| "未知错误".into())
            )));
        }
        let content = parsed
            .choices
            .and_then(|cs| cs.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .unwrap_or_default();
        let usage = parsed.usage.unwrap_or(OnceUsage {
            prompt_tokens: None,
            completion_tokens: None,
        });
        // 无 usage 字段时本地估算（AC-7）
        let tokens_in = usage
            .prompt_tokens
            .unwrap_or_else(|| self.estimate_tokens(&req.messages.iter().map(|m| m.content.as_str()).collect::<String>()));
        let tokens_out = usage
            .completion_tokens
            .unwrap_or_else(|| self.estimate_tokens(&content));
        Ok(ChatResponse {
            content,
            model: req.model.clone(),
            tokens_in,
            tokens_out,
            served_by: format!("{}.{}", self.provider_name, req.model),
        })
    }

    fn estimate_tokens(&self, text: &str) -> u32 {
        estimate_tokens(text)
    }

    fn pricing(&self, model: &str) -> Option<Price> {
        self.models
            .iter()
            .find(|m| m.id == model)
            .map(|m| Price {
                price_in: m.price_in,
                price_out: m.price_out,
            })
    }
}

/// tiktoken cl100k 估算（全模块共用）。
pub fn estimate_tokens(text: &str) -> u32 {
    match tiktoken_rs::cl100k_base() {
        Ok(bpe) => bpe.encode_with_special_tokens(text).len() as u32,
        Err(_) => (text.chars().count() as f64 * 0.6) as u32 + 1, // 兜底粗估
    }
}

/// 用量缺失时的本地估算。
pub fn estimate_usage(adapter: &dyn ProviderAdapter, req_text: &str, out_text: &str) -> Usage {
    Usage {
        tokens_in: adapter.estimate_tokens(req_text),
        tokens_out: adapter.estimate_tokens(out_text),
    }
}
