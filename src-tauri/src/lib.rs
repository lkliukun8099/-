//! 麦兜错题软件 Rust 核心库。
//!
//! 职责：Tauri Builder 装配、插件注册、全局状态注入、命令注册、
//! 启动后台任务（每日备份、复习到期扫描）。

pub mod ai;
pub mod attribution;
pub mod backup;
pub mod commands;
pub mod crypto;
pub mod db;
pub mod error;
pub mod kg;
pub mod logging;
pub mod mastery;
pub mod ocr;
pub mod review;
pub mod sync;
pub mod variant;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use error::AppError;

/// 全局应用状态（注入 Tauri State）。
pub struct AppState {
    /// 数据库连接池（SQLCipher）
    pub db: db::DbPool,
    /// AI 统一接入层（所有云端调用的唯一出口）
    pub gateway: Arc<ai::Gateway>,
    /// 应用数据目录（图片/备份/日志的落盘根）
    pub app_data_dir: PathBuf,
    /// 在线状态（Gateway 探测 + 系统事件双信号，§9-8）
    pub online: AtomicBool,
    /// 日志 WorkerGuard（持有以防异步日志丢失）
    pub log_guard: parking_lot::Mutex<Option<tracing_appender::non_blocking::WorkerGuard>>,
}

impl AppState {
    /// 是否在线（供各引擎判定走云端还是本地路径）
    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }

    /// 更新在线状态
    pub fn set_online(&self, online: bool) {
        self.online.store(online, Ordering::Relaxed);
    }
}

/// 应用入口（由 main.rs 调用）。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            use tauri::Manager;

            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| AppError::internal(format!("无法获取应用数据目录: {e}")))?;
            std::fs::create_dir_all(&app_data_dir)?;

            // 1. 日志系统（最优先，后续模块都要打日志）
            let log_guard = logging::init_tracing(app_data_dir.join("logs"))
                .map_err(|e| AppError::internal(format!("日志初始化失败: {e}")))?;

            // 2. 数据库（SQLCipher，密钥来自 OS 钥匙串）
            let db = db::DbPool::init(app_data_dir.join("maiduo.db"))?;
            tracing::info!("数据库初始化完成");

            // 3. 知识图谱种子数据（首次启动导入苏教版数学）
            kg::seed_loader::ensure_seeded(&db, app)?;
            tracing::info!("知识图谱种子检查完成");

            // 4. AI Gateway（路由/熔断/计量/密钥）
            let gateway = Arc::new(ai::Gateway::new(db.clone()));

            // 5. 默认在线（启动后由后台探测修正）
            let state = AppState {
                db: db.clone(),
                gateway,
                app_data_dir: app_data_dir.clone(),
                online: AtomicBool::new(true),
                log_guard: parking_lot::Mutex::new(Some(log_guard)),
            };
            app.manage(state);

            // 6. 后台任务：每日备份（保留 7 份）+ 复习到期扫描
            let db_for_backup = db.clone();
            let backup_dir = app_data_dir.join("backups");
            tauri::async_runtime::spawn(async move {
                backup::start_daily_backup(db_for_backup, backup_dir).await;
            });

            let db_for_review = db.clone();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                review::notify::start_review_watch(db_for_review, app_handle).await;
            });

            tracing::info!("麦兜错题软件启动完成");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 错题（F-01/02/05）
            commands::mistake_cmd::mistake_create_from_image,
            commands::mistake_cmd::mistake_list,
            commands::mistake_cmd::mistake_get,
            commands::mistake_cmd::mistake_update,
            commands::mistake_cmd::mistake_delete,
            commands::mistake_cmd::tag_list,
            commands::mistake_cmd::tag_create,
            commands::mistake_cmd::mistake_set_tags,
            // OCR（F-04）
            commands::ocr_cmd::ocr_run,
            // 归因（F-06）与冲突合并（F-16）
            commands::attribution_cmd::attribution_run,
            commands::chat_cmd::chat_analyze,
            commands::attribution_cmd::attribution_confirm,
            commands::attribution_cmd::conflict_list,
            commands::attribution_cmd::conflict_resolve,
            // 变式（F-08/09/15）
            commands::variant_cmd::variant_generate,
            commands::variant_cmd::variant_train_queue,
            commands::variant_cmd::variant_submit_attempt,
            // 知识图谱（F-07）
            commands::kg_cmd::kg_tree,
            commands::kg_cmd::kg_search,
            // 报告（F-13）
            commands::mastery_cmd::mastery_report,
            // 复习（F-14）
            commands::review_cmd::review_today,
            commands::review_cmd::review_complete,
            commands::review_cmd::review_upcoming,
            commands::review_cmd::review_config_get,
            commands::review_cmd::review_config_set,
            // 模型接入（F-10/11/19）
            commands::model_cmd::model_provider_list,
            commands::model_cmd::model_provider_upsert,
            commands::model_cmd::model_provider_test,
            commands::model_cmd::model_provider_delete,
            commands::model_cmd::model_key_rotate,
            commands::model_cmd::route_policy_list,
            commands::model_cmd::route_policy_upsert,
            commands::model_cmd::ai_chat,
            commands::model_cmd::cost_stats,
            // 系统（N-03/N-04）
            commands::settings_cmd::settings_get,
            commands::settings_cmd::settings_set,
            commands::settings_cmd::backup_run,
            commands::settings_cmd::backup_list,
            commands::settings_cmd::backup_restore,
            commands::settings_cmd::log_clear,
            commands::settings_cmd::network_status,
            // 同步预留（F-21 P1 桩）
            commands::sync_cmd::sync_push,
            commands::sync_cmd::sync_pull,
        ])
        .run(tauri::generate_context!())
        .expect("运行麦兜错题软件时发生致命错误");
}
