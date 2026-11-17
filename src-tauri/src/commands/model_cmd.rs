//! 模型接入命令（F-10/F-11/F-19）：provider 管理、密钥轮转、路由策略、
//! 统一聊天入口（流式走 Channel）、成本面板。

use tauri::ipc::Channel;
use tauri::State;

use crate::ai::adapter::{ChatMessage, ChatRequest, StreamChunk};
use crate::db::dao::log::CostReport;
use crate::db::dao::model::{ModelDao, ModelProvider, ProviderInput, RoutePolicy};
use crate::db::dao::log::LogDao;
use crate::error::AppError;
use crate::AppState;

/// 供应商列表（含密钥配置状态，不返回密钥本体）。
#[tauri::command(rename_all = "camelCase")]
pub fn model_provider_list(state: State<'_, AppState>) -> Result<Vec<ModelProvider>, AppError> {
    let conn = state.db.conn()?;
    let mut providers = ModelDao::new(&conn).list_providers()?;
    for p in &mut providers {
        p.has_key = state.gateway.keychain().has_key(&p.key_alias);
    }
    Ok(providers)
}

/// 新建/更新供应商；附带 api_key 时写入钥匙串（版本轮转）。
#[tauri::command(rename_all = "camelCase")]
pub fn model_provider_upsert(
    state: State<'_, AppState>,
    input: ProviderInput,
) -> Result<i64, AppError> {
    if input.name.trim().is_empty() || input.base_url.trim().is_empty() {
        return Err(AppError::invalid_param("供应商名称与 base_url 不能为空"));
    }
    let conn = state.db.conn()?;
    let dao = ModelDao::new(&conn);

    // key_alias：更新时沿用旧的，新建时按名称生成
    let key_alias = match input.id.and_then(|id| dao.provider_by_id(id).ok().flatten()) {
        Some(existing) => existing.key_alias,
        None => format!("provider-{}", input.name.trim().to_lowercase()),
    };

    // 密钥先落钥匙串（失败则整体失败，不产生无密钥配置）
    if let Some(key) = input.api_key.as_ref().filter(|k| !k.trim().is_empty()) {
        state.gateway.keychain().store_key(&key_alias, key.trim())?;
    }

    dao.upsert_provider(&input, &key_alias)
}

/// 连通性测试。
#[tauri::command(rename_all = "camelCase")]
pub async fn model_provider_test(
    state: State<'_, AppState>,
    provider_id: i64,
) -> Result<String, AppError> {
    let provider = {
        let conn = state.db.conn()?;
        ModelDao::new(&conn)
            .provider_by_id(provider_id)?
            .ok_or_else(|| AppError::not_found(format!("模型供应商 #{provider_id} 不存在")))?
    };
    state.gateway.test_provider(&provider).await
}

/// 删除供应商（同时删除钥匙串密钥）。
#[tauri::command(rename_all = "camelCase")]
pub fn model_provider_delete(state: State<'_, AppState>, provider_id: i64) -> Result<(), AppError> {
    let conn = state.db.conn()?;
    let dao = ModelDao::new(&conn);
    if let Some(p) = dao.provider_by_id(provider_id)? {
        let _ = state.gateway.keychain().delete_key(&p.key_alias);
    }
    dao.delete_provider(provider_id)
}

/// 密钥轮转（不改动 provider 配置，仅钥匙串版本 +1）。
#[tauri::command(rename_all = "camelCase")]
pub fn model_key_rotate(
    state: State<'_, AppState>,
    key_alias: String,
    api_key: String,
) -> Result<u32, AppError> {
    state.gateway.keychain().store_key(&key_alias, &api_key)
}

/// 路由策略列表。
#[tauri::command(rename_all = "camelCase")]
pub fn route_policy_list(state: State<'_, AppState>) -> Result<Vec<RoutePolicy>, AppError> {
    let conn = state.db.conn()?;
    ModelDao::new(&conn).list_policies()
}

/// 新建/更新路由策略。
#[tauri::command(rename_all = "camelCase")]
pub fn route_policy_upsert(
    state: State<'_, AppState>,
    policy: RoutePolicy,
) -> Result<i64, AppError> {
    if policy.primary.split_once('.').is_none() {
        return Err(AppError::invalid_param(
            "主模型格式应为 provider.model，如 deepseek.deepseek-chat",
        ));
    }
    let conn = state.db.conn()?;
    ModelDao::new(&conn).upsert_policy(&policy)
}

/// 统一聊天入口（走 Gateway，流式经 Channel 推送 StreamChunk）。
#[tauri::command(rename_all = "camelCase")]
pub async fn ai_chat(
    state: State<'_, AppState>,
    task_type: String,
    messages: Vec<ChatMessage>,
    channel: Channel<StreamChunk>,
) -> Result<(), AppError> {
    if messages.is_empty() {
        return Err(AppError::invalid_param("消息列表不能为空"));
    }
    let req = ChatRequest {
        model: String::new(), // 由路由层填充
        messages,
        temperature: None,
        max_tokens: None,
    };
    state.gateway.chat_stream(&task_type, req, channel).await
}

/// 成本面板（F-19）。
#[tauri::command(rename_all = "camelCase")]
pub fn cost_stats(
    state: State<'_, AppState>,
    range_days: Option<i64>,
) -> Result<CostReport, AppError> {
    let conn = state.db.conn()?;
    LogDao::new(&conn).cost_report(range_days.unwrap_or(30).clamp(1, 365))
}
