//! 每日自动备份（保留最近 7 份滚动删除）与恢复（N-04）。
//!
//! 备份采用 `VACUUM INTO`：在同一加密连接内导出，
//! 产物仍为以同一密钥加密的 SQLCipher 库（密文复制，不明文落地）。

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::db::DbPool;
use crate::error::AppError;

/// 备份文件信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub created_at: String,
}

/// 保留份数
const KEEP_COUNT: usize = 7;

/// 后台任务：启动即补一次当日备份，随后每 24h 执行一次。
pub async fn start_daily_backup(db: DbPool, backup_dir: PathBuf) {
    loop {
        if let Err(e) = backup_now(&db, &backup_dir) {
            tracing::warn!("自动备份失败: {e}");
        }
        if let Err(e) = prune_backups(&backup_dir) {
            tracing::warn!("备份清理失败: {e}");
        }
        tokio::time::sleep(std::time::Duration::from_secs(24 * 3600)).await;
    }
}

/// 立即执行一次备份（同日重复执行会覆盖当日文件）。
pub fn backup_now(db: &DbPool, backup_dir: &Path) -> Result<PathBuf, AppError> {
    std::fs::create_dir_all(backup_dir)?;
    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let target = backup_dir.join(format!("maiduo-{today}.db"));
    if target.exists() {
        std::fs::remove_file(&target)?;
    }
    let conn = db.conn()?;
    conn.execute(
        "VACUUM INTO ?1",
        [target.to_string_lossy().to_string()],
    )?;
    tracing::info!("数据库备份完成: {}", target.display());
    Ok(target)
}

/// 列出全部备份（新→旧）。
pub fn list_backups(backup_dir: &Path) -> Result<Vec<BackupInfo>, AppError> {
    let mut out = Vec::new();
    if !backup_dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(backup_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.starts_with("maiduo-") && name.ends_with(".db")) {
            continue;
        }
        let meta = entry.metadata()?;
        let created = meta
            .modified()
            .map(|t| {
                let dt: chrono::DateTime<chrono::Local> = t.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_default();
        out.push(BackupInfo {
            path: entry.path().to_string_lossy().to_string(),
            name,
            size_bytes: meta.len(),
            created_at: created,
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// 滚动清理：只保留最新 7 份。
fn prune_backups(backup_dir: &Path) -> Result<(), AppError> {
    let backups = list_backups(backup_dir)?;
    for old in backups.iter().skip(KEEP_COUNT) {
        std::fs::remove_file(&old.path)?;
        tracing::info!("清理过期备份: {}", old.name);
    }
    Ok(())
}

/// 从备份恢复：以备份文件覆盖当前库文件。
///
/// 注意：恢复后建议重启应用以确保全部连接使用新文件。
pub fn restore_backup(db: &DbPool, backup_path: &Path) -> Result<(), AppError> {
    if !backup_path.exists() {
        return Err(AppError::not_found(format!(
            "备份文件不存在: {}",
            backup_path.display()
        )));
    }
    // 目标路径（去掉 -wal/-shm 旧文件，避免 WAL 残留导致数据不一致）
    let db_path = db.path().clone();
    let wal = db_path.with_extension("db-wal");
    let shm = db_path.with_extension("db-shm");
    for aux in [&wal, &shm] {
        if aux.exists() {
            let _ = std::fs::remove_file(aux);
        }
    }
    std::fs::copy(backup_path, &db_path)
        .map_err(|e| AppError::backup(format!("恢复失败: {e}")))?;
    tracing::info!("已从备份恢复: {}", backup_path.display());
    Ok(())
}
