//! 复习排程（F-14）：类艾宾浩斯，间隔可配置（默认 1/3/7/15/30 天），答错重置。

pub mod notify;

use serde::Serialize;

use crate::db::dao::review::{ReviewDao, ReviewItem, ReviewPlan};
use crate::db::DbPool;
use crate::error::AppError;

/// 复习排程器。
pub struct ReviewScheduler {
    db: DbPool,
}

/// review_complete 的返回。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOutcome {
    /// 是否还有下一段复习（false 表示全部完成）
    pub has_next: bool,
    /// 下一段计划（答对推进 / 答错重置回第一段）
    pub next_plan: Option<ReviewPlan>,
    /// 当前阶段（0 起始）
    pub stage: i64,
}

impl ReviewScheduler {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    /// 为目标排入复习队列（stage 0 起，due = 今天 + intervals[0] 天）。
    pub fn schedule(&self, profile_id: i64, target_type: &str, target_id: i64) -> Result<i64, AppError> {
        let conn = self.db.conn()?;
        let dao = ReviewDao::new(&conn);
        let intervals = dao.intervals(profile_id)?;
        let first = intervals.first().copied().unwrap_or(1);
        // 时间口径统一 UTC 存储（前端本地时区展示，§9-4）
        let due = chrono::Utc::now() + chrono::Duration::days(first);
        dao.upsert_plan(
            profile_id,
            target_type,
            target_id,
            0,
            &due.format("%Y-%m-%d %H:%M:%S").to_string(),
        )
    }

    /// 完成一次复习：答对推进下一段，答错重置回第一段。
    pub fn complete(&self, plan_id: i64, is_correct: bool) -> Result<ReviewOutcome, AppError> {
        let conn = self.db.conn()?;
        let dao = ReviewDao::new(&conn);
        let plan = dao.get(plan_id)?;
        if plan.status != "pending" {
            return Err(AppError::invalid_param(format!(
                "复习计划 #{plan_id} 状态为 {}，无法完成",
                plan.status
            )));
        }
        dao.mark_done(plan_id)?;

        let intervals = dao.intervals(plan.profile_id)?;
        let next_stage = if is_correct { plan.stage + 1 } else { 0 };

        if next_stage >= intervals.len() as i64 {
            // 全部阶段完成
            return Ok(ReviewOutcome {
                has_next: false,
                next_plan: None,
                stage: plan.stage,
            });
        }

        let days = intervals[next_stage as usize];
        let due = chrono::Utc::now() + chrono::Duration::days(days);
        let next_id = dao.upsert_plan(
            plan.profile_id,
            &plan.target_type,
            plan.target_id,
            next_stage,
            &due.format("%Y-%m-%d %H:%M:%S").to_string(),
        )?;
        Ok(ReviewOutcome {
            has_next: true,
            next_plan: Some(dao.get(next_id)?),
            stage: next_stage,
        })
    }

    /// 今日待复习队列。
    pub fn today(&self, profile_id: i64) -> Result<Vec<ReviewItem>, AppError> {
        let conn = self.db.conn()?;
        let dao = ReviewDao::new(&conn);
        let plans = dao.due_today(profile_id)?;
        let mut items = Vec::with_capacity(plans.len());
        for plan in plans {
            let title = dao.target_title(&plan.target_type, plan.target_id)?;
            items.push(ReviewItem {
                plan,
                title,
                subject_name: None,
            });
        }
        Ok(items)
    }

    /// 未来 N 天计划（复习日历）。
    pub fn upcoming(&self, profile_id: i64, days: i64) -> Result<Vec<ReviewPlan>, AppError> {
        let conn = self.db.conn()?;
        ReviewDao::new(&conn).upcoming(profile_id, days)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::dao::review::ReviewDao;

    fn scheduler(tag: &str) -> ReviewScheduler {
        ReviewScheduler::new(DbPool::test_pool(tag).unwrap())
    }

    /// 解析 "YYYY-MM-DD HH:MM:SS" 与今天 UTC 日期的天数差（存储口径统一 UTC）。
    fn days_from_today(due: &str) -> i64 {
        let due_date = chrono::NaiveDate::parse_from_str(&due[..10], "%Y-%m-%d").unwrap();
        let today = chrono::Utc::now().date_naive();
        (due_date - today).num_days()
    }

    #[test]
    fn schedule_first_interval() {
        let s = scheduler("rev-sched");
        let id = s.schedule(1, "mistake", 100).unwrap();
        let conn = s.db.conn().unwrap();
        let plan = ReviewDao::new(&conn).get(id).unwrap();
        assert_eq!(plan.stage, 0);
        assert_eq!(plan.status, "pending");
        assert_eq!(days_from_today(&plan.next_due_at), 1, "默认间隔首段为 +1 天");
    }

    #[test]
    fn schedule_dedup_pending_plan() {
        let s = scheduler("rev-dedup");
        let id1 = s.schedule(1, "mistake", 100).unwrap();
        let id2 = s.schedule(1, "mistake", 100).unwrap();
        assert_eq!(id1, id2, "同一目标已有 pending 计划时不重复建");
    }

    #[test]
    fn complete_correct_advances_stage() {
        let s = scheduler("rev-adv");
        let id = s.schedule(1, "mistake", 100).unwrap();
        let out = s.complete(id, true).unwrap();
        assert!(out.has_next);
        assert_eq!(out.stage, 1);
        let next = out.next_plan.unwrap();
        assert_eq!(next.stage, 1);
        assert_eq!(days_from_today(&next.next_due_at), 3, "第二段为 +3 天（1/3/7/15/30）");
        // 旧计划已 done
        let conn = s.db.conn().unwrap();
        assert_eq!(ReviewDao::new(&conn).get(id).unwrap().status, "done");
    }

    #[test]
    fn complete_wrong_resets_to_first_stage() {
        let s = scheduler("rev-reset");
        let id = s.schedule(1, "mistake", 100).unwrap();
        let out = s.complete(id, true).unwrap(); // 推进到 stage 1
        let stage1_id = out.next_plan.unwrap().id;
        let out2 = s.complete(stage1_id, false).unwrap(); // 答错重置
        assert!(out2.has_next);
        assert_eq!(out2.stage, 0, "答错应重置回第一段");
        assert_eq!(days_from_today(&out2.next_plan.unwrap().next_due_at), 1);
    }

    #[test]
    fn complete_all_stages_finishes() {
        let s = scheduler("rev-finish");
        // 自定义短间隔 [1,3]
        {
            let conn = s.db.conn().unwrap();
            ReviewDao::new(&conn).set_intervals(1, &[1, 3]).unwrap();
        }
        let id = s.schedule(1, "mistake", 100).unwrap();
        let o1 = s.complete(id, true).unwrap();
        assert!(o1.has_next);
        let o2 = s.complete(o1.next_plan.unwrap().id, true).unwrap();
        assert!(!o2.has_next, "全部阶段完成后 has_next=false");
        assert!(o2.next_plan.is_none());
    }

    #[test]
    fn complete_non_pending_rejected() {
        let s = scheduler("rev-nopending");
        let id = s.schedule(1, "mistake", 100).unwrap();
        s.complete(id, true).unwrap(); // 已 done
        let err = s.complete(id, true).unwrap_err();
        assert_eq!(err.code, 1001, "重复完成应返回参数错误");
    }

    #[test]
    fn custom_intervals_roundtrip() {
        let s = scheduler("rev-intervals");
        {
            let conn = s.db.conn().unwrap();
            let dao = ReviewDao::new(&conn);
            dao.set_intervals(1, &[2, 5, 10]).unwrap();
            assert_eq!(dao.intervals(1).unwrap(), vec![2, 5, 10]);
        }
        let id = s.schedule(1, "mistake", 200).unwrap();
        let conn = s.db.conn().unwrap();
        let plan = ReviewDao::new(&conn).get(id).unwrap();
        assert_eq!(days_from_today(&plan.next_due_at), 2, "应使用自定义间隔");
    }

    #[test]
    fn malformed_intervals_fallback_default() {
        let s = scheduler("rev-badcfg");
        {
            let conn = s.db.conn().unwrap();
            // 直接写入非法 JSON，验证容错回退 [1,3,7,15,30]
            conn.execute(
                "INSERT INTO review_config(profile_id, intervals) VALUES (1, 'not-json')",
                [],
            )
            .unwrap();
            assert_eq!(ReviewDao::new(&conn).intervals(1).unwrap(), vec![1, 3, 7, 15, 30]);
        }
    }

    #[test]
    fn due_today_only_pending_and_due() {
        let s = scheduler("rev-today");
        // 直接插入一个已到期的计划
        {
            let conn = s.db.conn().unwrap();
            conn.execute(
                "INSERT INTO review_plan(profile_id, target_type, target_id, stage, next_due_at)
                 VALUES (1, 'mistake', 300, 0, datetime('now', '-1 hour'))",
                [],
            )
            .unwrap();
            // 一个明天才到期的（统一 UTC 口径，due_today 用 UTC 比较）
            conn.execute(
                "INSERT INTO review_plan(profile_id, target_type, target_id, stage, next_due_at)
                 VALUES (1, 'mistake', 301, 0, datetime('now', '+2 days'))",
                [],
            )
            .unwrap();
        }
        let items = s.today(1).unwrap();
        assert_eq!(items.len(), 1, "只有已到期 pending 计划进入今日队列");
        assert_eq!(items[0].plan.target_id, 300);
    }
}
