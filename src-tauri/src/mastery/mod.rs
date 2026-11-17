//! 掌握度模型（F-15）：EMA（指数滑动平均）更新 + 历史快照。
//!
//! 公式：score_new = score + α × (target − score)，α = 0.3，target ∈ {1, 0}；
//! 首次作答以 0.5 为先验（中性起步）。

use crate::db::dao::mastery::MasteryDao;
use crate::db::DbPool;
use crate::error::AppError;

/// EMA 平滑系数
const ALPHA: f64 = 0.3;
/// 首次作答先验分
const PRIOR: f64 = 0.5;

/// 作答后更新掌握度，返回分数变化量（新 − 旧）。
///
/// 同时写入 mastery_history 快照供掌握度曲线使用。
pub fn update_after_attempt(
    db: &DbPool,
    profile_id: i64,
    kg_node_id: i64,
    is_correct: bool,
) -> Result<f64, AppError> {
    let conn = db.conn()?;
    let dao = MasteryDao::new(&conn);
    let old_score = dao
        .get(profile_id, kg_node_id)?
        .map(|m| m.score)
        .unwrap_or(PRIOR);
    let target = if is_correct { 1.0 } else { 0.0 };
    let new_score = (old_score + ALPHA * (target - old_score)).clamp(0.0, 1.0);

    dao.upsert_score(profile_id, kg_node_id, new_score, is_correct)?;
    dao.record_history(profile_id, kg_node_id, new_score)?;
    Ok(new_score - old_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建测试库并插入一个三级知识节点（mastery 有外键约束）。
    fn setup(tag: &str) -> DbPool {
        let pool = DbPool::test_pool(tag).unwrap();
        let conn = pool.conn().unwrap();
        conn.execute(
            "INSERT INTO kg_node(subject_id, code, name, level) VALUES (1, 'M.T', '测试节点', 3)",
            [],
        )
        .unwrap();
        pool
    }

    #[test]
    fn first_correct_from_prior() {
        let pool = setup("mastery-c1");
        // 先验 0.5，答对 → 0.5 + 0.3×(1−0.5) = 0.65，delta = +0.15
        let delta = update_after_attempt(&pool, 1, 1, true).unwrap();
        assert!((delta - 0.15).abs() < 1e-9, "delta={delta}");
        let conn = pool.conn().unwrap();
        let m = MasteryDao::new(&conn).get(1, 1).unwrap().unwrap();
        assert!((m.score - 0.65).abs() < 1e-9);
        assert_eq!(m.attempts, 1);
        assert_eq!(m.correct, 1);
        assert!(m.last_assessed_at.is_some());
    }

    #[test]
    fn first_wrong_from_prior() {
        let pool = setup("mastery-w1");
        // 0.5 + 0.3×(0−0.5) = 0.35，delta = −0.15
        let delta = update_after_attempt(&pool, 1, 1, false).unwrap();
        assert!((delta + 0.15).abs() < 1e-9);
        let conn = pool.conn().unwrap();
        let m = MasteryDao::new(&conn).get(1, 1).unwrap().unwrap();
        assert!((m.score - 0.35).abs() < 1e-9);
        assert_eq!(m.correct, 0);
    }

    #[test]
    fn ema_converges_upward_on_streak() {
        let pool = setup("mastery-streak");
        let mut last = 0.5;
        for _ in 0..10 {
            update_after_attempt(&pool, 1, 1, true).unwrap();
            let conn = pool.conn().unwrap();
            last = MasteryDao::new(&conn).get(1, 1).unwrap().unwrap().score;
        }
        // 连对 10 次后应显著高于先验（0.5→~0.972）
        assert!(last > 0.9, "连对 10 次后 score={last} 应 >0.9");
        let conn = pool.conn().unwrap();
        let m = MasteryDao::new(&conn).get(1, 1).unwrap().unwrap();
        assert_eq!(m.attempts, 10);
        assert_eq!(m.correct, 10);
    }

    #[test]
    fn history_snapshot_written_each_attempt() {
        let pool = setup("mastery-hist");
        update_after_attempt(&pool, 1, 1, true).unwrap();
        update_after_attempt(&pool, 1, 1, false).unwrap();
        let conn = pool.conn().unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM mastery_history WHERE profile_id = 1 AND kg_node_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "每次作答应写一条历史快照（F-13 曲线数据源）");
    }

    #[test]
    fn score_bounded_0_1() {
        let pool = setup("mastery-bound");
        for _ in 0..50 {
            update_after_attempt(&pool, 1, 1, true).unwrap();
        }
        let conn = pool.conn().unwrap();
        let score = MasteryDao::new(&conn).get(1, 1).unwrap().unwrap().score;
        assert!((0.0..=1.0).contains(&score));
    }
}
