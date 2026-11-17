//! 诊断报告命令（F-13）。

use tauri::State;

use crate::db::dao::mastery::{MasteryDao, ReportDto};
use crate::db::dao::mistake::MistakeDao;
use crate::error::AppError;
use crate::kg;
use crate::AppState;

/// 诊断报告：热力图 + 掌握度曲线 + 错因分布。
#[tauri::command(rename_all = "camelCase")]
pub fn mastery_report(
    state: State<'_, AppState>,
    profile_id: Option<i64>,
    range_days: Option<i64>,
) -> Result<ReportDto, AppError> {
    let profile_id = profile_id.unwrap_or(1);
    let range_days = range_days.unwrap_or(30).clamp(1, 365);
    let conn = state.db.conn()?;
    let dao = MasteryDao::new(&conn);

    let mut heatmap = dao.heatmap(profile_id, 60)?;
    // 回填节点路径
    for cell in &mut heatmap {
        cell.node_path = kg::node_path(&conn, cell.kg_node_id);
    }

    let (_, _, total_mistakes) = MistakeDao::new(&conn).counts_for_home(profile_id)?;

    Ok(ReportDto {
        heatmap,
        curve: dao.curve(profile_id, range_days)?,
        cause_distribution: dao.cause_distribution(profile_id)?,
        total_mistakes,
        reviewed_today: dao.reviewed_today(profile_id)?,
    })
}
