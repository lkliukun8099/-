//! 复习到期本地通知（F-14）+ 过期计划清理后台任务。

use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

use crate::db::dao::review::ReviewDao;
use crate::db::DbPool;

/// 后台任务：每小时扫描一次到期复习计划并发送本地通知；
/// 顺带清理过期 7 天未完成的计划。
pub async fn start_review_watch(db: DbPool, app: tauri::AppHandle) {
    // 启动后先等 30s，避免与启动流程争抢资源
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    loop {
        if let Err(e) = scan_and_notify(&db, &app) {
            tracing::warn!("复习到期扫描失败: {e}");
        }
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

/// 扫描到期计划并通知（同一轮只发一条汇总通知，避免打扰）。
fn scan_and_notify(db: &DbPool, app: &tauri::AppHandle) -> Result<(), crate::error::AppError> {
    let conn = db.conn()?;
    let dao = ReviewDao::new(&conn);

    // 单档案首版：profile_id = 1
    let due = dao.due_today(1)?;
    if !due.is_empty() {
        let notified_key = format!("review_notified_{}", chrono::Utc::now().format("%Y%m%d"));
        let already: bool = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = ?1",
                [&notified_key],
                |r| r.get::<_, String>(0),
            )
            .map(|v| v == "1")
            .unwrap_or(false);
        if !already {
            app.notification()
                .builder()
                .title("麦兜错题 · 今日复习提醒")
                .body(format!("今天有 {} 道错题到期待复习，趁热打铁！", due.len()))
                .show()
                .map_err(|e| {
                    crate::error::AppError::internal(format!("通知发送失败: {e}"))
                })?;
            conn.execute(
                "INSERT INTO app_meta(key, value) VALUES (?1, '1')
                 ON CONFLICT(key) DO UPDATE SET value = '1'",
                [&notified_key],
            )?;
            tracing::info!("复习提醒已发送：{} 项到期", due.len());
        }
    }

    // 过期清理（7 天未完成标记 expired）
    let expired = dao.expire_overdue(7)?;
    if expired > 0 {
        tracing::info!("清理过期复习计划 {expired} 条");
    }
    Ok(())
}

// 保证 Manager trait 被链接（部分通知 API 路径需要）
#[allow(unused)]
fn _assert_manager<T: Manager>(_t: &T) {}
