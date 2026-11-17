//! 感知层：OCR 引擎抽象 + 双引擎编排 + 一致性校验（F-04）。
//!
//! 引擎矩阵（架构 §1.1）：
//! - 云端公式识别（SimpleTex/Mathpix 类，在线主力，输出 LaTeX）
//! - Tesseract sidecar（离线文字）
//! - ONNX 公式模型（可选下载，离线公式，feature=onnx）

pub mod cloud_ocr;
pub mod onnx_formula;
pub mod preprocess;
pub mod tesseract_ocr;

use std::path::{Path, PathBuf};
use std::time::Instant;

use async_trait::async_trait;
use serde::Serialize;

use crate::db::dao::log::LogDao;
use crate::db::dao::mistake::MistakeDao;
use crate::db::DbPool;
use crate::error::AppError;
use crate::AppState;

/// OCR 统一结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    pub text: String,
    pub latex: Option<String>,
    /// cloud / tesseract / onnx / mixed
    pub engine: String,
    pub confidence: f64,
    /// 双引擎一致性得分（0~1，单引擎时为 None）
    pub consistency: Option<f64>,
    pub latency_ms: i64,
}

/// OCR 引擎抽象。
#[async_trait]
pub trait OcrEngine: Send + Sync {
    fn name(&self) -> &str;
    async fn recognize(&self, image_path: &Path) -> Result<OcrResult, AppError>;
}

/// OCR 偏好（命令参数）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrPrefer {
    Auto,
    Cloud,
    Offline,
}

impl OcrPrefer {
    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "auto" => Ok(Self::Auto),
            "cloud" => Ok(Self::Cloud),
            "offline" => Ok(Self::Offline),
            _ => Err(AppError::invalid_param(format!(
                "OCR 偏好仅支持 auto/cloud/offline，收到: {s}"
            ))),
        }
    }
}

/// OCR 编排器：双引擎调度 + 一致性比对 + 结果落库。
pub struct OcrOrchestrator {
    db: DbPool,
    app_data_dir: PathBuf,
    online: bool,
}

impl OcrOrchestrator {
    pub fn new(state: &AppState) -> Self {
        Self {
            db: state.db.clone(),
            app_data_dir: state.app_data_dir.clone(),
            online: state.is_online(),
        }
    }

    /// 分部件构造（后台任务等无法持有 &AppState 的场景）。
    pub fn for_parts(db: DbPool, app_data_dir: PathBuf, online: bool) -> Self {
        Self {
            db,
            app_data_dir,
            online,
        }
    }

    /// 主编排入口。
    pub async fn run(&self, mistake_id: i64, prefer: OcrPrefer) -> Result<OcrResult, AppError> {
        let conn = self.db.conn()?;
        let mistake = MistakeDao::new(&conn).get(mistake_id)?;
        let rel = mistake
            .processed_image
            .clone()
            .unwrap_or(mistake.source_image.clone());
        let image_path = preprocess::absolute_path(&self.app_data_dir, &rel);
        if !image_path.exists() {
            return Err(AppError::not_found(format!(
                "错题图片不存在: {}",
                image_path.display()
            )));
        }

        let use_cloud = match prefer {
            OcrPrefer::Cloud => true,
            OcrPrefer::Offline => false,
            OcrPrefer::Auto => self.online,
        };

        let result = if use_cloud {
            match self.run_cloud_with_cross_check(mistake_id, &image_path).await {
                Ok(r) => r,
                Err(e) => {
                    // 云端失败自动降级本地（N-04）
                    tracing::warn!("云端 OCR 失败，降级本地引擎: {e}");
                    self.run_offline(mistake_id, &image_path).await?
                }
            }
        } else {
            self.run_offline(mistake_id, &image_path).await?
        };

        // 回写错题表
        MistakeDao::new(&conn).update_ocr_result(
            mistake_id,
            &result.text,
            result.latex.as_deref(),
            &result.engine,
            result.confidence,
        )?;
        Ok(result)
    }

    /// 在线路径：云端公式识别 + Tesseract 交叉验证。
    async fn run_cloud_with_cross_check(
        &self,
        mistake_id: i64,
        image_path: &Path,
    ) -> Result<OcrResult, AppError> {
        let cloud = cloud_ocr::CloudFormulaOcr::from_settings(&self.db)?;
        let start = Instant::now();
        let cloud_result = cloud.recognize(image_path).await?;
        let cloud_latency = start.elapsed().as_millis() as i64;
        self.record_job(mistake_id, &cloud_result, cloud_latency);

        // 本地 Tesseract 交叉（失败不影响主结果）
        let tess = tesseract_ocr::TesseractOcr::locate();
        let (consistency, final_confidence) = if let Ok(engine) = tess {
            let t_start = Instant::now();
            match engine.recognize(image_path).await {
                Ok(local) => {
                    self.record_job(
                        mistake_id,
                        &local,
                        t_start.elapsed().as_millis() as i64,
                    );
                    let sim = consistency_check(&cloud_result.text, &local.text);
                    // 一致性高则提升置信度，低则保守取较低值
                    let conf = if sim >= 0.8 {
                        cloud_result.confidence.max(0.9)
                    } else {
                        cloud_result.confidence * sim.max(0.5)
                    };
                    (Some(sim), conf)
                }
                Err(e) => {
                    tracing::warn!("Tesseract 交叉验证失败（忽略）: {e}");
                    (None, cloud_result.confidence)
                }
            }
        } else {
            (None, cloud_result.confidence)
        };

        Ok(OcrResult {
            consistency,
            confidence: final_confidence,
            latency_ms: cloud_latency,
            ..cloud_result
        })
    }

    /// 离线路径：Tesseract 文字 + （可选）ONNX 公式。
    async fn run_offline(
        &self,
        mistake_id: i64,
        image_path: &Path,
    ) -> Result<OcrResult, AppError> {
        let tess = tesseract_ocr::TesseractOcr::locate()?;
        let start = Instant::now();
        let mut text_result = tess.recognize(image_path).await?;
        let text_latency = start.elapsed().as_millis() as i64;
        self.record_job(mistake_id, &text_result, text_latency);

        // ONNX 公式识别（模型存在时）
        let onnx = onnx_formula::OnnxFormulaOcr::new(&self.app_data_dir);
        if onnx.is_available() {
            let f_start = Instant::now();
            match onnx.recognize(image_path).await {
                Ok(formula) => {
                    self.record_job(
                        mistake_id,
                        &formula,
                        f_start.elapsed().as_millis() as i64,
                    );
                    text_result.latex = formula.latex;
                    text_result.engine = "mixed".to_string();
                }
                Err(e) => tracing::warn!("ONNX 公式识别失败（降级为仅文字）: {e}"),
            }
        }

        text_result.latency_ms = text_latency;
        Ok(text_result)
    }

    /// OCR 任务留痕（ocr_job 表）。
    fn record_job(&self, mistake_id: i64, result: &OcrResult, latency_ms: i64) {
        let _ = self.db.conn().map(|conn| {
            let _ = LogDao::new(&conn).insert_ocr_job(
                mistake_id,
                &result.engine,
                "done",
                Some(&result.text),
                result.latex.as_deref(),
                Some(result.confidence),
                Some(latency_ms),
            );
        });
    }
}

/// 双引擎一致性校验：归一化编辑距离相似度（0~1）。
pub fn consistency_check(a: &str, b: &str) -> f64 {
    let a = normalize(a);
    let b = normalize(b);
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let dist = levenshtein(&a, &b);
    let max_len = a.chars().count().max(b.chars().count());
    if max_len == 0 {
        return 1.0;
    }
    1.0 - (dist as f64 / max_len as f64)
}

/// 归一化：去空白、全角转半角常见符号。
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '（' => '(',
            '）' => ')',
            '，' => ',',
            '。' => '.',
            '：' => ':',
            other => other,
        })
        .collect()
}

/// Levenshtein 编辑距离（字符级，滚动数组）。
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            curr[j + 1] = (prev[j] + usize::from(ca != cb))
                .min(prev[j + 1] + 1)
                .min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// 供命令层复用的状态克隆辅助。
pub fn orchestrator_from(state: &AppState) -> OcrOrchestrator {
    OcrOrchestrator::new(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consistency_identical() {
        assert_eq!(consistency_check("二次函数 x²", "二次函数 x²"), 1.0);
    }

    #[test]
    fn consistency_both_empty() {
        assert_eq!(consistency_check("", ""), 1.0);
        assert_eq!(consistency_check("  ", "　"), 1.0, "纯空白归一化后等价");
    }

    #[test]
    fn consistency_totally_different() {
        assert_eq!(consistency_check("abc", "xyz"), 0.0);
    }

    #[test]
    fn consistency_whitespace_and_fullwidth_normalized() {
        assert_eq!(consistency_check("a b，c", "ab,c"), 1.0);
    }

    #[test]
    fn consistency_partial() {
        // 1 字差异（替换）：dist=1，max_len=8 → 0.875
        let sim = consistency_check("二次函数单调区间", "二次函数单调区问");
        assert!((sim - 0.875).abs() < 1e-9, "sim={sim}");
    }

    #[test]
    fn levenshtein_correctness() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("相同", "相同"), 0);
        // 对称性
        assert_eq!(levenshtein("ab", "ba"), levenshtein("ba", "ab"));
    }

    #[test]
    fn ocr_prefer_parse() {
        assert_eq!(OcrPrefer::parse("auto").unwrap(), OcrPrefer::Auto);
        assert_eq!(OcrPrefer::parse("cloud").unwrap(), OcrPrefer::Cloud);
        assert_eq!(OcrPrefer::parse("offline").unwrap(), OcrPrefer::Offline);
        assert_eq!(OcrPrefer::parse("weird").unwrap_err().code, 1001);
    }
}
