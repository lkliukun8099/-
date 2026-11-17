//! Tesseract sidecar 离线文字识别（跨平台免编译，随包分发）。
//!
//! 定位顺序：`MAIDUO_TESSERACT_PATH` 环境变量 → 可执行文件同目录 `binaries/tesseract(.exe)`
//! → 系统 PATH。均不可用时报 3002（离线 OCR 不可用）。

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::{OcrEngine, OcrResult};
use crate::error::AppError;

/// Tesseract sidecar 引擎。
pub struct TesseractOcr {
    bin_path: PathBuf,
}

impl TesseractOcr {
    pub fn new(bin_path: PathBuf) -> Self {
        Self { bin_path }
    }

    /// 按约定顺序定位 sidecar 二进制。
    pub fn locate() -> Result<Self, AppError> {
        let exe_name = if cfg!(windows) {
            "tesseract.exe"
        } else {
            "tesseract"
        };

        // 1. 环境变量覆盖（开发/自定义部署）
        if let Ok(custom) = std::env::var("MAIDUO_TESSERACT_PATH") {
            let p = PathBuf::from(custom);
            if p.exists() {
                return Ok(Self::new(p));
            }
        }
        // 2. 可执行文件同目录 binaries/
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for candidate in [
                    dir.join("binaries").join(exe_name),
                    // Tauri sidecar 命名：{name}-{target_triple}[.exe]
                    dir.join("binaries").join(format!(
                        "tesseract-{}",
                        target_triple_suffix()
                    )),
                ] {
                    if candidate.exists() {
                        return Ok(Self::new(candidate));
                    }
                }
            }
        }
        // 3. 系统 PATH
        if which_on_path(exe_name) {
            return Ok(Self::new(PathBuf::from(exe_name)));
        }
        Err(AppError::ocr_unavailable(
            "未找到 Tesseract sidecar（离线 OCR 不可用）",
        ))
    }
}

/// 当前目标三元组后缀（sidecar 命名约定）。
fn target_triple_suffix() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc.exe"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown"
    }
}

/// PATH 上是否存在可执行文件。
fn which_on_path(exe_name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(exe_name);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

#[async_trait]
impl OcrEngine for TesseractOcr {
    fn name(&self) -> &str {
        "tesseract"
    }

    async fn recognize(&self, image_path: &Path) -> Result<OcrResult, AppError> {
        // tesseract <image> stdout -l chi_sim+eng --psm 6
        let output = tokio::process::Command::new(&self.bin_path)
            .arg(image_path)
            .arg("stdout")
            .arg("-l")
            .arg("chi_sim+eng")
            .arg("--psm")
            .arg("6")
            .output()
            .await
            .map_err(|e| AppError::ocr(format!("Tesseract 启动失败: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::ocr(format!(
                "Tesseract 识别失败: {}",
                stderr.chars().take(200).collect::<String>()
            )));
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            return Err(AppError::ocr("Tesseract 未识别出文字"));
        }
        // Tesseract 无置信度输出（tsv 模式可取），离线路径给保守常量
        Ok(OcrResult {
            text,
            latex: None,
            engine: self.name().to_string(),
            confidence: 0.7,
            consistency: None,
            latency_ms: 0,
        })
    }
}
