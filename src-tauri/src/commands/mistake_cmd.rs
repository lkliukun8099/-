//! 错题命令（F-01/02/05）：录入、列表、详情、编辑、删除、标签。

use std::sync::Arc;

use tauri::State;

use crate::attribution::{AttributionEngine, AttributionMode};
use crate::db::dao::attribution::AttributionDao;
use crate::db::dao::mistake::{
    Mistake, MistakeDao, MistakeDetail, MistakePatch, MistakeQuery, MistakeSummary, Tag,
};
use crate::db::dao::review::ReviewDao;
use crate::db::dao::variant::VariantDao;
use crate::db::dao::Page;
use crate::error::AppError;
use crate::kg;
use crate::ocr::preprocess::{self, ImageRegion};
use crate::ocr::{OcrPrefer, OcrOrchestrator};
use crate::AppState;

/// 从图片创建错题：落盘 → 入库 → 后台触发 OCR → 归因流水线（F-01/02）。
///
/// `image_path`：前端圈选/校正后的临时图片绝对路径；
/// `region`：可选的归一化圈选区域（前端未裁切时由后端裁）。
#[tauri::command(rename_all = "camelCase")]
pub async fn mistake_create_from_image(
    state: State<'_, AppState>,
    image_path: String,
    region: Option<ImageRegion>,
) -> Result<Mistake, AppError> {
    let src = std::path::PathBuf::from(&image_path);
    if !src.exists() {
        return Err(AppError::not_found(format!("图片不存在: {image_path}")));
    }
    // 1. 图像落盘（格式统一为 PNG，可按区域裁切）
    let relative = preprocess::persist_image(&state.app_data_dir, &src, region)?;

    // 2. 入库
    let mistake_id = {
        let conn = state.db.conn()?;
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, &relative)?;
        id
    };

    // 3. 后台流水线：OCR → 归因（auto），不阻塞录入返回（G1：录入 ≤15s）
    let state_clone = Arc::new(AppStateRef {
        db: state.db.clone(),
        gateway: state.gateway.clone(),
        app_data_dir: state.app_data_dir.clone(),
        online: state.is_online(),
    });
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_pipeline(state_clone, mistake_id).await {
            tracing::warn!("错题 #{mistake_id} 自动流水线失败: {e}");
        }
    });

    let conn = state.db.conn()?;
    MistakeDao::new(&conn).get(mistake_id)
}

/// 后台流水线所需的最小状态（AppState 不可直接移入 spawn）。
struct AppStateRef {
    db: crate::db::DbPool,
    gateway: Arc<crate::ai::Gateway>,
    app_data_dir: std::path::PathBuf,
    online: bool,
}

/// OCR → 归因 自动流水线。
async fn run_pipeline(state: Arc<AppStateRef>, mistake_id: i64) -> Result<(), AppError> {
    // OCR
    let orchestrator = OcrOrchestrator::for_parts(
        state.db.clone(),
        state.app_data_dir.clone(),
        state.online,
    );
    orchestrator.run(mistake_id, OcrPrefer::Auto).await?;

    // 归因（auto：在线 cloud / 离线 local）
    let engine = AttributionEngine::new(state.db.clone(), state.gateway.clone());
    let mode = if state.online {
        AttributionMode::Cloud
    } else {
        AttributionMode::Local
    };
    if let Err(e) = engine.analyze(mistake_id, mode).await {
        // 归因失败不阻塞 OCR 结果，仅记日志
        tracing::warn!("错题 #{mistake_id} 自动归因失败: {e}");
    }
    Ok(())
}

/// 多维筛选 + FTS 检索 + 分页（F-05）。
#[tauri::command(rename_all = "camelCase")]
pub fn mistake_list(
    state: State<'_, AppState>,
    query: MistakeQuery,
) -> Result<Page<MistakeSummary>, AppError> {
    let conn = state.db.conn()?;
    MistakeDao::new(&conn).list(&query)
}

/// 错题详情（含归因/变式/标签/复习记录）。
#[tauri::command(rename_all = "camelCase")]
pub fn mistake_get(state: State<'_, AppState>, id: i64) -> Result<MistakeDetail, AppError> {
    let conn = state.db.conn()?;
    let mistake = MistakeDao::new(&conn).get(id)?;
    let mut attributions = AttributionDao::new(&conn).list_for_mistake(id)?;
    // 回填图谱路径
    for attr in &mut attributions {
        if let Some(nid) = attr.kg_node_id {
            attr.kg_path = kg::node_path(&conn, nid);
        }
    }
    Ok(MistakeDetail {
        mistake,
        attributions,
        variants: VariantDao::new(&conn).list_for_mistake(id)?,
        tags: MistakeDao::new(&conn).tags_of_mistake(id)?,
        review_plans: ReviewDao::new(&conn).plans_for_target("mistake", id)?,
    })
}

/// 编辑错题（乐观锁）。
#[tauri::command(rename_all = "camelCase")]
pub fn mistake_update(
    state: State<'_, AppState>,
    id: i64,
    patch: MistakePatch,
) -> Result<(), AppError> {
    let conn = state.db.conn()?;
    MistakeDao::new(&conn).update(id, &patch)
}

/// 删除错题（软删除）。
#[tauri::command(rename_all = "camelCase")]
pub fn mistake_delete(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    let conn = state.db.conn()?;
    MistakeDao::new(&conn).soft_delete(id)
}

/// 标签列表。
#[tauri::command(rename_all = "camelCase")]
pub fn tag_list(state: State<'_, AppState>) -> Result<Vec<Tag>, AppError> {
    let conn = state.db.conn()?;
    MistakeDao::new(&conn).list_tags(1)
}

/// 新建标签。
#[tauri::command(rename_all = "camelCase")]
pub fn tag_create(
    state: State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> Result<i64, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::invalid_param("标签名不能为空"));
    }
    let conn = state.db.conn()?;
    MistakeDao::new(&conn).create_tag(1, name.trim(), color.as_deref())
}

/// 设置错题标签（覆盖式）。
#[tauri::command(rename_all = "camelCase")]
pub fn mistake_set_tags(
    state: State<'_, AppState>,
    mistake_id: i64,
    tag_ids: Vec<i64>,
) -> Result<(), AppError> {
    let conn = state.db.conn()?;
    MistakeDao::new(&conn).set_mistake_tags(mistake_id, &tag_ids)
}
