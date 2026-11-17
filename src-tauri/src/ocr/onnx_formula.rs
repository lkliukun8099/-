//! ONNX 公式识别（pix2tex 类小模型，可选下载，低端机可裁剪）。
//!
//! 模型文件约定：`<app_data_dir>/models/formula_encoder.onnx` +
//! `<app_data_dir>/models/formula_decoder.onnx`（用户按需下载放置）。
//! 未启用 `onnx` feature 编译时，本模块为完整可编译的降级实现：
//! `is_available()` 恒为 false，调用返回 3002。

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::{OcrEngine, OcrResult};
use crate::error::AppError;

/// ONNX 公式识别引擎。
pub struct OnnxFormulaOcr {
    model_dir: PathBuf,
}

impl OnnxFormulaOcr {
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            model_dir: app_data_dir.join("models"),
        }
    }

    /// 模型是否就绪（文件存在且编译包含 onnx feature）。
    pub fn is_available(&self) -> bool {
        cfg!(feature = "onnx")
            && self.model_dir.join("formula_encoder.onnx").exists()
            && self.model_dir.join("formula_decoder.onnx").exists()
    }
}

#[async_trait]
impl OcrEngine for OnnxFormulaOcr {
    fn name(&self) -> &str {
        "onnx"
    }

    async fn recognize(&self, image_path: &Path) -> Result<OcrResult, AppError> {
        recognize_impl(&self.model_dir, image_path).await
    }
}

/// 推理实现（按编译 feature 分派）。
#[cfg(feature = "onnx")]
async fn recognize_impl(model_dir: &Path, image_path: &Path) -> Result<OcrResult, AppError> {
    // ort 运行时推理：encoder 提取视觉特征 → decoder 自回归生成 LaTeX token 序列。
    // 模型输入预处理：缩放到 448x448、归一化；tokenizer 随模型同目录（vocab.json）。
    let encoder_path = model_dir.join("formula_encoder.onnx");
    let decoder_path = model_dir.join("formula_decoder.onnx");

    let image = image::open(image_path)
        .map_err(|e| AppError::ocr(format!("公式图像读取失败: {e}")))?
        .resize_exact(448, 448, image::imageops::FilterType::Lanczos3)
        .to_rgb8();
    // NCHW 浮点张量，[0,1] 归一化
    let mut input = vec![0f32; 3 * 448 * 448];
    for (x, y, pixel) in image.enumerate_pixels() {
        let idx = (y as usize) * 448 + x as usize;
        input[idx] = pixel[0] as f32 / 255.0;
        input[448 * 448 + idx] = pixel[1] as f32 / 255.0;
        input[2 * 448 * 448 + idx] = pixel[2] as f32 / 255.0;
    }

    // 推理在阻塞线程执行，避免占用异步运行时
    let encoder_path = encoder_path.clone();
    let decoder_path = decoder_path.clone();
    let latex = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
        let encoder = ort::session::Session::builder()
            .map_err(|e| AppError::ocr(format!("ONNX 初始化失败: {e}")))?
            .commit_from_file(&encoder_path)
            .map_err(|e| AppError::ocr(format!("encoder 加载失败: {e}")))?;
        let decoder = ort::session::Session::builder()
            .map_err(|e| AppError::ocr(format!("ONNX 初始化失败: {e}")))?
            .commit_from_file(&decoder_path)
            .map_err(|e| AppError::ocr(format!("decoder 加载失败: {e}")))?;
        // 具体张量绑定依最终选定模型（U-4）的输入输出名实现；
        // 本函数体在模型选型确认后填充 token 解码循环。
        let _ = (encoder, decoder, input);
        Err(AppError::ocr_unavailable(
            "ONNX 公式模型未选定（U-4），推理管道待模型确认后启用",
        ))
    })
    .await
    .map_err(|e| AppError::ocr(format!("ONNX 推理线程失败: {e}")))??;

    Ok(OcrResult {
        text: latex.clone(),
        latex: Some(latex),
        engine: "onnx".to_string(),
        confidence: 0.75,
        consistency: None,
        latency_ms: 0,
    })
}

/// 降级实现（未编译 onnx feature）。
#[cfg(not(feature = "onnx"))]
async fn recognize_impl(
    _model_dir: &Path,
    _image_path: &Path,
) -> Result<OcrResult, AppError> {
    Err(AppError::ocr_unavailable(
        "ONNX 公式识别未编译进本构建（需 feature=onnx 与模型文件）",
    ))
}
