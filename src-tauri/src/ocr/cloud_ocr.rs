//! 云端公式 OCR（SimpleTex/Mathpix 类 API，供应商配置化，架构 U-3）。
//!
//! 配置来源（用户在系统设置中填写）：
//! - `app_meta.ocr_cloud_base_url`：API 端点（POST multipart: file 字段）
//! - 钥匙串别名 `ocr-cloud`：API Key
//!
//! 响应约定（适配 SimpleTex 风格）：`{"latex": "...", "text": "...", "confidence": 0.97}`

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;

use super::{OcrEngine, OcrResult};
use crate::ai::keychain::Keychain;
use crate::db::DbPool;
use crate::error::AppError;

/// 云端 OCR 密钥别名
pub const OCR_KEY_ALIAS: &str = "ocr-cloud";

/// 云端公式 OCR 引擎。
pub struct CloudFormulaOcr {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, serde::Deserialize)]
struct CloudResp {
    latex: Option<String>,
    text: Option<String>,
    confidence: Option<f64>,
    // Mathpix 风格兼容字段
    #[serde(default)]
    error: Option<String>,
}

impl CloudFormulaOcr {
    pub fn new(base_url: String, api_key: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .expect("HTTP 客户端构建失败");
        Self {
            http,
            base_url,
            api_key,
        }
    }

    /// 从设置装配（未配置返回参数错误，由编排器降级）。
    pub fn from_settings(db: &DbPool) -> Result<Self, AppError> {
        let conn = db.conn()?;
        let base_url: Option<String> = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'ocr_cloud_base_url'",
                [],
                |r| r.get(0),
            )
            .ok();
        let base_url = base_url
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| AppError::ocr_unavailable("未配置云端 OCR 端点（系统设置 → ocr_cloud_base_url）"))?;
        let api_key = Keychain::new()
            .get_key(OCR_KEY_ALIAS)?
            .ok_or_else(|| AppError::ocr_unavailable("未配置云端 OCR API Key（别名 ocr-cloud）"))?;
        Ok(Self::new(base_url, api_key))
    }
}

#[async_trait]
impl OcrEngine for CloudFormulaOcr {
    fn name(&self) -> &str {
        "cloud"
    }

    async fn recognize(&self, image_path: &Path) -> Result<OcrResult, AppError> {
        let bytes = std::fs::read(image_path)?;
        let file_name = image_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "image.png".to_string());
        let part = reqwest::multipart::Part::bytes(bytes).file_name(file_name);
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("formats", "text,latex");

        let resp = self
            .http
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(AppError::ocr(format!(
                "云端 OCR HTTP {status}: {}",
                body.chars().take(200).collect::<String>()
            )));
        }
        let parsed: CloudResp = serde_json::from_str(&body)?;
        if let Some(err) = parsed.error {
            return Err(AppError::ocr(format!("云端 OCR 错误: {err}")));
        }
        let latex = parsed.latex.filter(|s| !s.is_empty());
        // 无独立 text 字段时以 latex 作为文本兜底
        let text = parsed
            .text
            .filter(|s| !s.is_empty())
            .or_else(|| latex.clone())
            .ok_or_else(|| AppError::ocr("云端 OCR 返回为空"))?;
        Ok(OcrResult {
            text,
            latex,
            engine: self.name().to_string(),
            confidence: parsed.confidence.unwrap_or(0.9),
            consistency: None,
            latency_ms: 0, // 由编排器统一计时
        })
    }
}
