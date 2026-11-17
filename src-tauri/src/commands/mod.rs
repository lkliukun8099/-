//! Tauri Commands 层：一模块一文件，统一返回 `Result<T, AppError>`。

pub mod attribution_cmd;
pub mod kg_cmd;
pub mod mastery_cmd;
pub mod mistake_cmd;
pub mod model_cmd;
pub mod ocr_cmd;
pub mod review_cmd;
pub mod settings_cmd;
pub mod sync_cmd;
pub mod variant_cmd;
