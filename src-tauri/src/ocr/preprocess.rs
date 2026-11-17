//! 图像落盘与格式转换（前端 OpenCV 已做边缘检测/透视校正，
//! Rust 侧仅做格式统一与存储管理，架构 §1.3 职责划分）。

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::AppError;

/// 圈选区域（归一化坐标 0~1）。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 将源图片（任意常见格式）转为 PNG 存入应用数据目录，
/// 可按归一化区域裁切。返回相对路径（`images/xxx.png`）。
pub fn persist_image(
    app_data_dir: &Path,
    src_path: &Path,
    region: Option<ImageRegion>,
) -> Result<String, AppError> {
    let img = image::open(src_path)
        .map_err(|e| AppError::invalid_param(format!("图片读取失败: {e}")))?;

    let img = match region {
        Some(r) => {
            let w = img.width();
            let h = img.height();
            // 归一化坐标转像素并夹取
            let x = ((r.x * w as f64) as u32).min(w.saturating_sub(1));
            let y = ((r.y * h as f64) as u32).min(h.saturating_sub(1));
            let cw = ((r.width * w as f64) as u32).clamp(1, w - x);
            let ch = ((r.height * h as f64) as u32).clamp(1, h - y);
            image::DynamicImage::ImageRgba8(image::imageops::crop_imm(&img, x, y, cw, ch).to_image())
        }
        None => img,
    };

    let dir = app_data_dir.join("images");
    std::fs::create_dir_all(&dir)?;
    let file_name = format!("{}.png", uuid::Uuid::new_v4());
    let target = dir.join(&file_name);
    img.save(&target)
        .map_err(|e| AppError::internal(format!("图片保存失败: {e}")))?;
    Ok(format!("images/{file_name}"))
}

/// 相对路径转绝对路径。
pub fn absolute_path(app_data_dir: &Path, relative: &str) -> PathBuf {
    app_data_dir.join(relative)
}
