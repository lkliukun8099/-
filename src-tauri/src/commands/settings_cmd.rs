//! 系统设置命令：应用元信息、备份、日志清理、在线状态（N-03/N-04）。

use tauri::State;

use crate::backup::{self, BackupInfo};
use crate::db::dao::log::LogDao;
use crate::error::AppError;
use crate::AppState;

/// 读取设置项（app_meta）。
#[tauri::command(rename_all = "camelCase")]
pub fn settings_get(state: State<'_, AppState>, key: String) -> Result<Option<String>, AppError> {
    let conn = state.db.conn()?;
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = ?1",
            [&key],
            |r| r.get(0),
        )
        .ok();
    Ok(value)
}

/// 写入设置项。
#[tauri::command(rename_all = "camelCase")]
pub fn settings_set(state: State<'_, AppState>, key: String, value: String) -> Result<(), AppError> {
    if key.trim().is_empty() {
        return Err(AppError::invalid_param("设置键不能为空"));
    }
    let conn = state.db.conn()?;
    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [&key, &value],
    )?;
    Ok(())
}

/// 立即执行一次备份，返回备份文件路径。
#[tauri::command(rename_all = "camelCase")]
pub fn backup_run(state: State<'_, AppState>) -> Result<String, AppError> {
    let dir = state.app_data_dir.join("backups");
    let path = backup::backup_now(&state.db, &dir)?;
    Ok(path.to_string_lossy().to_string())
}

/// 列出全部备份。
#[tauri::command(rename_all = "camelCase")]
pub fn backup_list(state: State<'_, AppState>) -> Result<Vec<BackupInfo>, AppError> {
    let dir = state.app_data_dir.join("backups");
    backup::list_backups(&dir)
}

/// 从指定备份恢复（恢复后建议重启应用）。
#[tauri::command(rename_all = "camelCase")]
pub fn backup_restore(state: State<'_, AppState>, backup_path: String) -> Result<(), AppError> {
    backup::restore_backup(&state.db, std::path::Path::new(&backup_path))
}

/// 一键清除：调用日志（call_log）+ 磁盘日志文件（隐私，N-03）。
/// 返回清除的调用日志条数。
#[tauri::command(rename_all = "camelCase")]
pub fn log_clear(state: State<'_, AppState>) -> Result<i64, AppError> {
    let conn = state.db.conn()?;
    let cleared = LogDao::new(&conn).clear_call_logs()? as i64;

    // 删除滚动日志文件
    let log_dir = state.app_data_dir.join("logs");
    if log_dir.exists() {
        for entry in std::fs::read_dir(&log_dir)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("maiduo.log") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    tracing::info!("用户执行日志一键清除，删除调用日志 {cleared} 条");
    Ok(cleared)
}

/// 当前在线状态（前端徽章轮询兜底）。
#[tauri::command(rename_all = "camelCase")]
pub fn network_status(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(state.is_online())
}
