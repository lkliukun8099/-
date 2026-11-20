//! 腾讯混元适配器：TC3-HMAC-SHA256 签名协议（架构 §2.1：签名单独适配）。
//!
//! 说明：混元也已提供 OpenAI 兼容端点，若 provider.protocol 配置为
//! `openai_compat` 则走通用适配器；本适配器对应 `protocol = "hunyuan_tc3"`。
//! 密钥格式约定：`secret_id:secret_key`（冒号分隔存入钥匙串）。
//! 流式：v1 实现将一次性响应包装为单分块流（协议差异，功能等价）。

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::ai::adapter::{
    ChatRequest, ChatResponse, ChatStream, Price, ProviderAdapter, StreamChunk, Usage,
};
use crate::db::dao::model::ModelSpec;
use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

const SERVICE: &str = "hunyuan";
const HOST: &str = "hunyuan.tencentcloudapi.com";
const ACTION: &str = "ChatCompletions";
const VERSION: &str = "2023-09-01";
const ALGORITHM: &str = "TC3-HMAC-SHA256";

/// 混元 TC3 签名适配器。
pub struct HunyuanAdapter {
    http: reqwest::Client,
    provider_name: String,
    secret_id: String,
    secret_key: String,
    models: Vec<ModelSpec>,
}

#[derive(Debug, serde::Deserialize)]
struct HyResp {
    #[serde(rename = "Response")]
    response: HyResponseBody,
}

#[derive(Debug, serde::Deserialize)]
struct HyResponseBody {
    #[serde(rename = "Choices")]
    choices: Option<Vec<HyChoice>>,
    #[serde(rename = "Usage")]
    usage: Option<HyUsage>,
    #[serde(rename = "Error")]
    error: Option<HyError>,
}

#[derive(Debug, serde::Deserialize)]
struct HyChoice {
    #[serde(rename = "Message")]
    message: Option<HyMessage>,
}

#[derive(Debug, serde::Deserialize)]
struct HyMessage {
    #[serde(rename = "Content")]
    content: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct HyUsage {
    #[serde(rename = "PromptTokens")]
    prompt_tokens: Option<u32>,
    #[serde(rename = "CompletionTokens")]
    completion_tokens: Option<u32>,
}

#[derive(Debug, serde::Deserialize)]
struct HyError {
    #[serde(rename = "Message")]
    message: Option<String>,
}

impl HunyuanAdapter {
    /// 构造：api_key 形如 `secret_id:secret_key`。
    pub fn new(
        http: reqwest::Client,
        provider_name: String,
        api_key: String,
        models: Vec<ModelSpec>,
    ) -> Result<Self, AppError> {
        let (id, key) = api_key.split_once(':').ok_or_else(|| {
            AppError::invalid_param("混元密钥格式应为 secret_id:secret_key")
        })?;
        Ok(Self {
            http,
            provider_name,
            secret_id: id.to_string(),
            secret_key: key.to_string(),
            models,
        })
    }

    fn hmac_sha256(key: &[u8], data: &str) -> Vec<u8> {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC 接受任意长度密钥");
        mac.update(data.as_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    fn sha256_hex(data: &str) -> String {
        crate::db::hex_encode(&Sha256::digest(data.as_bytes()))
    }

    /// TC3-HMAC-SHA256 签名，生成 Authorization 头。
    fn sign(&self, payload: &str, timestamp: i64, date: &str) -> String {
        // 1. 规范请求串
        let canonical_request = format!(
            "POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:{HOST}\nx-tc-action:{}\n\ncontent-type;host;x-tc-action\n{}",
            ACTION.to_lowercase(),
            Self::sha256_hex(payload)
        );
        // 2. 待签名字符串
        let credential_scope = format!("{date}/{SERVICE}/tc3_request");
        let string_to_sign = format!(
            "{ALGORITHM}\n{timestamp}\n{credential_scope}\n{}",
            Self::sha256_hex(&canonical_request)
        );
        // 3. 计算签名
        let secret_date = Self::hmac_sha256(format!("TC3{}", self.secret_key).as_bytes(), date);
        let secret_service = Self::hmac_sha256(&secret_date, SERVICE);
        let secret_signing = Self::hmac_sha256(&secret_service, "tc3_request");
        let signature = crate::db::hex_encode(&Self::hmac_sha256(&secret_signing, &string_to_sign));

        format!(
            "{ALGORITHM} Credential={}/{}, SignedHeaders=content-type;host;x-tc-action, Signature={signature}",
            self.secret_id, credential_scope
        )
    }

    /// 发起一次混元调用。
    async fn call(&self, req: &ChatRequest) -> Result<ChatResponse, AppError> {
        // 混元消息结构：Role/Content 大写
        let messages: Vec<serde_json::Value> = req
            .messages
            .iter()
            .map(|m| serde_json::json!({"Role": m.role, "Content": m.content}))
            .collect();
        let payload = serde_json::json!({
            "Model": req.model,
            "Messages": messages,
            "Stream": false,
        })
        .to_string();

        let timestamp = chrono::Utc::now().timestamp();
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let authorization = self.sign(&payload, timestamp, &date);

        let resp = self
            .http
            .post(format!("https://{HOST}/"))
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Host", HOST)
            .header("X-TC-Action", ACTION)
            .header("X-TC-Version", VERSION)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("Authorization", authorization)
            .body(payload)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(AppError::ai_call(format!(
                "hunyuan HTTP {status}: {}",
                body.chars().take(300).collect::<String>()
            )));
        }
        let parsed: HyResp = serde_json::from_str(&body)?;
        if let Some(err) = parsed.response.error {
            return Err(AppError::ai_call(format!(
                "hunyuan: {}",
                err.message.unwrap_or_else(|| "未知错误".into())
            )));
        }
        let content = parsed
            .response
            .choices
            .and_then(|cs| cs.into_iter().next())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .unwrap_or_default();
        let usage = parsed.response.usage;
        let tokens_in = usage
            .as_ref()
            .and_then(|u| u.prompt_tokens)
            .unwrap_or_else(|| self.estimate_tokens(&req.messages.iter().map(|m| m.content.as_str()).collect::<String>()));
        let tokens_out = usage
            .and_then(|u| u.completion_tokens)
            .unwrap_or_else(|| self.estimate_tokens(&content));
        Ok(ChatResponse {
            content,
            model: req.model.clone(),
            tokens_in,
            tokens_out,
            served_by: format!("{}.{}", self.provider_name, req.model),
        })
    }
}

#[async_trait]
impl ProviderAdapter for HunyuanAdapter {
    fn name(&self) -> &str {
        &self.provider_name
    }

    async fn chat_stream(&self, req: ChatRequest) -> Result<ChatStream, AppError> {
        // v1：一次性调用包装为单分块流（协议差异，功能等价）
        let resp = self.call(&req).await?;
        Ok(Box::pin(async_stream::stream! {
            yield Ok(StreamChunk::delta(resp.content));
            yield Ok(StreamChunk::done(
                Some(Usage {
                    tokens_in: resp.tokens_in,
                    tokens_out: resp.tokens_out,
                }),
                None,
            ));
        }))
    }

    async fn chat_once(&self, req: ChatRequest) -> Result<ChatResponse, AppError> {
        self.call(&req).await
    }

    fn estimate_tokens(&self, text: &str) -> u32 {
        super::openai_compat::estimate_tokens(text)
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
