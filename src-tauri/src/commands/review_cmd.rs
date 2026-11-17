//! 复习计划命令（F-14）。

use tauri::State;

use crate::db::dao::review::ReviewItem;
use crate::error::AppError;
use crate::review::{ReviewOutcome, ReviewScheduler};
use crate::AppState;

/// 今日待复习队列。
#[tauri::command(rename_all = "camelCase")]
pub fn review_today(
    state: State<'_, AppState>,
    profile_id: Option<i64>,
) -> Result<Vec<ReviewItem>, AppError> {
    ReviewScheduler::new(state.db.clone()).today(profile_id.unwrap_or(1))
}

/// 完成一次复习（答对推进 / 答错重置）。
#[tauri::command(rename_all = "camelCase")]
pub fn review_complete(
    state: State<'_, AppState>,
    plan_id: i64,
    is_correct: bool,
) -> Result<ReviewOutcome, AppError> {
    ReviewScheduler::new(state.db.clone()).complete(plan_id, is_correct)
}

/// 未来 N 天复习计划（复习日历用）。
#[tauri::command(rename_all = "camelCase")]
pub fn review_upcoming(
    state: State<'_, AppState>,
    profile_id: Option<i64>,
    days: Option<i64>,
) -> Result<Vec<crate::db::dao::review::ReviewPlan>, AppError> {
    ReviewScheduler::new(state.db.clone()).upcoming(
        profile_id.unwrap_or(1),
        days.unwrap_or(60).clamp(1, 365),
    )
}

/// 读取排程间隔配置（天数数组）。
#[tauri::command(rename_all = "camelCase")]
pub fn review_config_get(
    state: State<'_, AppState>,
    profile_id: Option<i64>,
) -> Result<Vec<i64>, AppError> {
    let conn = state.db.conn()?;
    crate::db::dao::review::ReviewDao::new(&conn).intervals(profile_id.unwrap_or(1))
}

/// 更新排程间隔配置（如 [1,3,7,15,30]）。
#[tauri::command(rename_all = "camelCase")]
pub fn review_config_set(
    state: State<'_, AppState>,
    intervals: Vec<i64>,
    profile_id: Option<i64>,
) -> Result<(), AppError> {
    if intervals.is_empty() || intervals.iter().any(|d| *d <= 0 || *d > 365) {
        return Err(AppError::invalid_param(
            "间隔必须为非空的 1-365 天数组",
        ));
    }
    let conn = state.db.conn()?;
    crate::db::dao::review::ReviewDao::new(&conn).set_intervals(
        profile_id.unwrap_or(1),
        &intervals,
    )
}
