//! 错题 DAO：主表 CRUD + 多维筛选 + FTS 检索 + 标签。

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use super::Page;
use crate::error::AppError;

/// 错题完整行（DTO，serde camelCase 与 TS 镜像）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mistake {
    pub id: i64,
    pub profile_id: i64,
    pub subject_id: Option<i64>,
    pub subject_name: Option<String>,
    pub source_image: String,
    pub processed_image: Option<String>,
    pub ocr_text: Option<String>,
    pub ocr_latex: Option<String>,
    pub ocr_engine: Option<String>,
    pub ocr_confidence: Option<f64>,
    pub student_answer: Option<String>,
    pub correct_answer: Option<String>,
    pub chapter: Option<String>,
    pub question_type: Option<String>,
    pub difficulty: i64,
    pub source: Option<String>,
    pub status: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 列表页摘要（避免全字段传输）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MistakeSummary {
    pub id: i64,
    pub subject_id: Option<i64>,
    pub subject_name: Option<String>,
    pub ocr_text: Option<String>,
    pub status: String,
    pub chapter: Option<String>,
    pub question_type: Option<String>,
    pub difficulty: i64,
    pub source_image: String,
    pub tags: Vec<String>,
    pub created_at: String,
}

/// 详情聚合：错题 + 归因历史 + 变式 + 标签 + 复习计划。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MistakeDetail {
    pub mistake: Mistake,
    pub attributions: Vec<super::attribution::Attribution>,
    pub variants: Vec<super::variant::VariantQuestion>,
    pub tags: Vec<Tag>,
    pub review_plans: Vec<super::review::ReviewPlan>,
}

/// 多维筛选查询（F-05）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MistakeQuery {
    pub subject_id: Option<i64>,
    pub status: Option<String>,
    pub tag_id: Option<i64>,
    pub keyword: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
}

fn default_page() -> i64 {
    1
}
fn default_page_size() -> i64 {
    20
}

/// 自定义标签。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
}

/// 错题可编辑字段（mistake_update 用）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MistakePatch {
    pub subject_id: Option<i64>,
    pub ocr_text: Option<String>,
    pub ocr_latex: Option<String>,
    pub student_answer: Option<String>,
    pub correct_answer: Option<String>,
    pub chapter: Option<String>,
    pub question_type: Option<String>,
    pub difficulty: Option<i64>,
    pub source: Option<String>,
    pub status: Option<String>,
    /// 乐观锁：必须与库中当前版本一致
    pub version: i64,
}

pub struct MistakeDao<'a> {
    conn: &'a Connection,
}

impl<'a> MistakeDao<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_mistake(row: &Row) -> rusqlite::Result<Mistake> {
        Ok(Mistake {
            id: row.get("id")?,
            profile_id: row.get("profile_id")?,
            subject_id: row.get("subject_id")?,
            subject_name: row.get("subject_name").ok(),
            source_image: row.get("source_image")?,
            processed_image: row.get("processed_image")?,
            ocr_text: row.get("ocr_text")?,
            ocr_latex: row.get("ocr_latex")?,
            ocr_engine: row.get("ocr_engine")?,
            ocr_confidence: row.get("ocr_confidence")?,
            student_answer: row.get("student_answer")?,
            correct_answer: row.get("correct_answer")?,
            chapter: row.get("chapter")?,
            question_type: row.get("question_type")?,
            difficulty: row.get("difficulty")?,
            source: row.get("source")?,
            status: row.get("status")?,
            version: row.get("version")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }

    const SELECT_FULL: &'static str =
        "SELECT m.*, s.name AS subject_name FROM mistake m
         LEFT JOIN subject s ON s.id = m.subject_id";

    /// 新建错题（拍照/导入录入，初始状态 pending_analysis）。
    pub fn insert(&self, profile_id: i64, source_image: &str) -> Result<i64, AppError> {
        self.conn.execute(
            "INSERT INTO mistake(profile_id, source_image) VALUES (?1, ?2)",
            params![profile_id, source_image],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 按 ID 取完整错题。
    pub fn get(&self, id: i64) -> Result<Mistake, AppError> {
        self.conn
            .query_row(
                &format!("{} WHERE m.id = ?1 AND m.deleted = 0", Self::SELECT_FULL),
                params![id],
                Self::row_to_mistake,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::not_found(format!("错题 #{id} 不存在"))
                }
                other => other.into(),
            })
    }

    /// 多维筛选 + FTS 全文检索 + 分页（万级 ≤300ms，AC-10）。
    pub fn list(&self, q: &MistakeQuery) -> Result<Page<MistakeSummary>, AppError> {
        let mut where_clauses = vec!["m.deleted = 0".to_string()];
        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(sid) = q.subject_id {
            where_clauses.push("m.subject_id = ?".into());
            param_values.push(Box::new(sid));
        }
        if let Some(status) = &q.status {
            where_clauses.push("m.status = ?".into());
            param_values.push(Box::new(status.clone()));
        }
        if let Some(tag_id) = q.tag_id {
            where_clauses.push(
                "m.id IN (SELECT mistake_id FROM mistake_tag_rel WHERE tag_id = ?)".into(),
            );
            param_values.push(Box::new(tag_id));
        }
        if let Some(kw) = q.keyword.as_ref().filter(|k| !k.trim().is_empty()) {
            let kw = kw.trim();
            if kw.chars().count() >= 3 {
                // FTS5 trigram 检索（≥3 字符支持子串命中）；双引号包裹防特殊字符注入
                let escaped = kw.replace('"', " ");
                where_clauses.push(
                    "m.id IN (SELECT rowid FROM mistake_fts WHERE mistake_fts MATCH ?)".into(),
                );
                param_values.push(Box::new(format!("\"{escaped}\"")));
            } else {
                // trigram 无法匹配 <3 字符，降级 LIKE 子串（转义通配符）
                let escaped = kw.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
                where_clauses.push("m.ocr_text LIKE ? ESCAPE '\\'".into());
                param_values.push(Box::new(format!("%{escaped}%")));
            }
        }
        if let Some(from) = &q.date_from {
            where_clauses.push("m.created_at >= ?".into());
            param_values.push(Box::new(from.clone()));
        }
        if let Some(to) = &q.date_to {
            where_clauses.push("m.created_at <= ?".into());
            param_values.push(Box::new(format!("{to} 23:59:59")));
        }

        let where_sql = where_clauses.join(" AND ");
        let page = q.page.max(1);
        let page_size = q.page_size.clamp(1, 100);
        let offset = (page - 1) * page_size;

        // 总数
        let count_sql = format!("SELECT COUNT(*) FROM mistake m WHERE {where_sql}");
        let mut params_ref: Vec<&dyn rusqlite::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let total: i64 = self
            .conn
            .query_row(&count_sql, params_ref.as_slice(), |r| r.get(0))?;

        // 列表（覆盖索引 + 分页）
        let list_sql = format!(
            "SELECT m.id, m.subject_id, s.name AS subject_name, m.ocr_text, m.status,
                    m.chapter, m.question_type, m.difficulty, m.source_image, m.created_at
             FROM mistake m LEFT JOIN subject s ON s.id = m.subject_id
             WHERE {where_sql} ORDER BY m.created_at DESC LIMIT ? OFFSET ?"
        );
        params_ref.push(&page_size);
        params_ref.push(&offset);
        let mut stmt = self.conn.prepare(&list_sql)?;
        let mut items: Vec<MistakeSummary> = stmt
            .query_map(params_ref.as_slice(), |row| {
                Ok(MistakeSummary {
                    id: row.get("id")?,
                    subject_id: row.get("subject_id")?,
                    subject_name: row.get("subject_name").ok(),
                    ocr_text: row.get("ocr_text")?,
                    status: row.get("status")?,
                    chapter: row.get("chapter")?,
                    question_type: row.get("question_type")?,
                    difficulty: row.get("difficulty")?,
                    source_image: row.get("source_image")?,
                    tags: Vec::new(), // 下方批量填充
                    created_at: row.get("created_at")?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // 批量填充标签（避免 N+1）
        if !items.is_empty() {
            let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
            let tag_map = self.tags_for_mistakes(&ids)?;
            for item in &mut items {
                item.tags = tag_map
                    .iter()
                    .filter(|(mid, _)| *mid == item.id)
                    .map(|(_, name)| name.clone())
                    .collect();
            }
        }

        Ok(Page::new(items, total, page, page_size))
    }

    /// 批量取若干错题的标签名。
    fn tags_for_mistakes(&self, ids: &[i64]) -> Result<Vec<(i64, String)>, AppError> {
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT r.mistake_id, t.name FROM mistake_tag_rel r
             JOIN tag t ON t.id = r.tag_id WHERE r.mistake_id IN ({placeholders})"
        );
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|i| i as &dyn rusqlite::ToSql).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params.as_slice(), |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 编辑错题（乐观锁：version 不一致返回 2004）。
    pub fn update(&self, id: i64, patch: &MistakePatch) -> Result<(), AppError> {
        let affected = self.conn.execute(
            "UPDATE mistake SET
               subject_id    = COALESCE(?1, subject_id),
               ocr_text      = COALESCE(?2, ocr_text),
               ocr_latex     = COALESCE(?3, ocr_latex),
               student_answer= COALESCE(?4, student_answer),
               correct_answer= COALESCE(?5, correct_answer),
               chapter       = COALESCE(?6, chapter),
               question_type = COALESCE(?7, question_type),
               difficulty    = COALESCE(?8, difficulty),
               source        = COALESCE(?9, source),
               status        = COALESCE(?10, status),
               version       = version + 1,
               updated_at    = datetime('now')
             WHERE id = ?11 AND deleted = 0 AND version = ?12",
            params![
                patch.subject_id,
                patch.ocr_text,
                patch.ocr_latex,
                patch.student_answer,
                patch.correct_answer,
                patch.chapter,
                patch.question_type,
                patch.difficulty,
                patch.source,
                patch.status,
                id,
                patch.version,
            ],
        )?;
        if affected == 0 {
            return Err(AppError::version_conflict(
                "数据已被其他操作修改，请刷新后重试",
            ));
        }
        Ok(())
    }

    /// 软删除。
    pub fn soft_delete(&self, id: i64) -> Result<(), AppError> {
        let affected = self.conn.execute(
            "UPDATE mistake SET deleted = 1, updated_at = datetime('now') WHERE id = ?1 AND deleted = 0",
            params![id],
        )?;
        if affected == 0 {
            return Err(AppError::not_found(format!("错题 #{id} 不存在")));
        }
        Ok(())
    }

    /// OCR 完成后回写识别结果。
    pub fn update_ocr_result(
        &self,
        id: i64,
        text: &str,
        latex: Option<&str>,
        engine: &str,
        confidence: f64,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE mistake SET ocr_text = ?1, ocr_latex = ?2, ocr_engine = ?3,
               ocr_confidence = ?4, version = version + 1, updated_at = datetime('now')
             WHERE id = ?5",
            params![text, latex, engine, confidence, id],
        )?;
        Ok(())
    }

    /// 更新状态（pending_analysis/analyzing/analyzed/conflict/archived）。
    pub fn update_status(&self, id: i64, status: &str) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE mistake SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    /// 待深度分析的错题（离线→在线批量深判，场景 C）。
    pub fn list_pending_analysis(&self, profile_id: i64) -> Result<Vec<Mistake>, AppError> {
        let mut stmt = self.conn.prepare(&format!(
            "{} WHERE m.profile_id = ?1 AND m.deleted = 0 AND m.status = 'pending_analysis'
             ORDER BY m.created_at",
            Self::SELECT_FULL
        ))?;
        let rows = stmt
            .query_map(params![profile_id], Self::row_to_mistake)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 今日面板计数：待分析/冲突/总数。
    pub fn counts_for_home(&self, profile_id: i64) -> Result<(i64, i64, i64), AppError> {
        let pending: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM mistake WHERE profile_id=?1 AND deleted=0 AND status='pending_analysis'",
            params![profile_id], |r| r.get(0))?;
        let conflict: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM mistake WHERE profile_id=?1 AND deleted=0 AND status='conflict'",
            params![profile_id], |r| r.get(0))?;
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM mistake WHERE profile_id=?1 AND deleted=0",
            params![profile_id], |r| r.get(0))?;
        Ok((pending, conflict, total))
    }

    // ---- 标签 ----

    /// 创建标签（已存在则返回已有 ID）。
    pub fn create_tag(&self, profile_id: i64, name: &str, color: Option<&str>) -> Result<i64, AppError> {
        self.conn.execute(
            "INSERT INTO tag(profile_id, name, color) VALUES (?1, ?2, ?3)
             ON CONFLICT(profile_id, name) DO NOTHING",
            params![profile_id, name, color],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM tag WHERE profile_id = ?1 AND name = ?2",
            params![profile_id, name],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    /// 档案下全部标签。
    pub fn list_tags(&self, profile_id: i64) -> Result<Vec<Tag>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, color FROM tag WHERE profile_id = ?1 ORDER BY name")?;
        let rows = stmt
            .query_map(params![profile_id], |r| {
                Ok(Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 单条错题的标签。
    pub fn tags_of_mistake(&self, mistake_id: i64) -> Result<Vec<Tag>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.name, t.color FROM tag t
             JOIN mistake_tag_rel r ON r.tag_id = t.id WHERE r.mistake_id = ?1 ORDER BY t.name",
        )?;
        let rows = stmt
            .query_map(params![mistake_id], |r| {
                Ok(Tag {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 覆盖式设置错题标签。
    pub fn set_mistake_tags(&self, mistake_id: i64, tag_ids: &[i64]) -> Result<(), AppError> {
        self.conn.execute(
            "DELETE FROM mistake_tag_rel WHERE mistake_id = ?1",
            params![mistake_id],
        )?;
        for tag_id in tag_ids {
            self.conn.execute(
                "INSERT OR IGNORE INTO mistake_tag_rel(mistake_id, tag_id) VALUES (?1, ?2)",
                params![mistake_id, tag_id],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbPool;

    fn dao_fixture() -> Connection {
        DbPool::memory_conn().unwrap()
    }

    fn query(page: i64, page_size: i64) -> MistakeQuery {
        MistakeQuery {
            subject_id: None,
            status: None,
            tag_id: None,
            keyword: None,
            date_from: None,
            date_to: None,
            page,
            page_size,
        }
    }

    #[test]
    fn insert_get_roundtrip() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, "images/a.png").unwrap();
        let m = dao.get(id).unwrap();
        assert_eq!(m.source_image, "images/a.png");
        assert_eq!(m.status, "pending_analysis", "新错题初始状态");
        assert_eq!(m.version, 1);
        assert_eq!(m.difficulty, 3);
    }

    #[test]
    fn get_missing_returns_1002() {
        let conn = dao_fixture();
        let err = MistakeDao::new(&conn).get(999).unwrap_err();
        assert_eq!(err.code, 1002);
    }

    #[test]
    fn list_pagination() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        for _ in 0..25 {
            dao.insert(1, "images/x.png").unwrap();
        }
        let p1 = dao.list(&query(1, 10)).unwrap();
        assert_eq!(p1.total, 25);
        assert_eq!(p1.items.len(), 10);
        let p3 = dao.list(&query(3, 10)).unwrap();
        assert_eq!(p3.items.len(), 5);
        // 页码钳制
        let p0 = dao.list(&query(0, 10)).unwrap();
        assert_eq!(p0.page, 1);
    }

    #[test]
    fn list_filter_by_status() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let a = dao.insert(1, "images/a.png").unwrap();
        dao.insert(1, "images/b.png").unwrap();
        dao.update_status(a, "analyzed").unwrap();
        let mut q = query(1, 20);
        q.status = Some("analyzed".into());
        let page = dao.list(&q).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, a);
    }

    #[test]
    fn fts_keyword_search_latin() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, "images/a.png").unwrap();
        dao.update_ocr_result(id, "solving quadratic equation practice", None, "tesseract", 0.9)
            .unwrap();
        let mut q = query(1, 20);
        q.keyword = Some("quadratic".into());
        let page = dao.list(&q).unwrap();
        assert_eq!(page.total, 1, "FTS5 拉丁词干检索应命中");
        // FTS 特殊字符注入防护：引号被剥离而不是报错
        q.keyword = Some("\" OR 1=1 --".into());
        assert!(dao.list(&q).is_ok());
    }

    /// 中文子串检索（B3 已修复：V2 迁移 trigram 分词 + DAO 层 <3 字符降级 LIKE）。
    #[test]
    fn fts_keyword_search_chinese_substring() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, "images/a.png").unwrap();
        dao.update_ocr_result(id, "已知函数在区间上单调递减", None, "tesseract", 0.9)
            .unwrap();
        let mut q = query(1, 20);
        q.keyword = Some("单调".into());
        let page = dao.list(&q).unwrap();
        assert_eq!(page.total, 1, "中文子串应命中（trigram 改造后）");
    }

    /// 混合策略边界（QA 补充）：≥3 字符走 trigram FTS，<3 字符降级 LIKE。
    #[test]
    fn hybrid_search_strategy_boundary() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, "images/a.png").unwrap();
        dao.update_ocr_result(id, "已知函数在区间上单调递减", None, "tesseract", 0.9)
            .unwrap();
        let mut q = query(1, 20);
        // ≥3 字符（trigram FTS MATCH）
        q.keyword = Some("单调递减".into());
        assert_eq!(dao.list(&q).unwrap().total, 1, "≥3 字符中文子串应命中");
        q.keyword = Some("区间上单".into());
        assert_eq!(dao.list(&q).unwrap().total, 1, "跨词子串应命中");
        q.keyword = Some("单调递增".into());
        assert_eq!(dao.list(&q).unwrap().total, 0, "不存在的子串不应命中");
        // 1 字符（LIKE 降级）
        q.keyword = Some("函".into());
        assert_eq!(dao.list(&q).unwrap().total, 1, "1 字符应走 LIKE 降级命中");
    }

    /// LIKE 降级路径的通配符转义（QA 补充：防 % 与 _ 被当通配符）。
    /// 注意用例均为 <3 字符以确保走 LIKE 分支。
    #[test]
    fn like_wildcards_are_escaped() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, "images/a.png").unwrap();
        dao.update_ocr_result(id, "增长率 50% 的应用题", None, "tesseract", 0.9)
            .unwrap();
        let mut q = query(1, 20);
        q.keyword = Some("0%".into());
        assert_eq!(dao.list(&q).unwrap().total, 1, "字面 0% 应命中（% 被转义）");
        q.keyword = Some("%".into());
        assert_eq!(dao.list(&q).unwrap().total, 1, "字面 % 应命中而非全匹配");
        q.keyword = Some("5_".into());
        assert_eq!(dao.list(&q).unwrap().total, 0, "下划线应被转义而非作通配符");
    }

    #[test]
    fn update_optimistic_lock() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, "images/a.png").unwrap();
        let ok_patch = MistakePatch {
            subject_id: None,
            ocr_text: None,
            ocr_latex: None,
            student_answer: None,
            correct_answer: None,
            chapter: Some("第一章".into()),
            question_type: None,
            difficulty: None,
            source: None,
            status: None,
            version: 1,
        };
        dao.update(id, &ok_patch).unwrap();
        let m = dao.get(id).unwrap();
        assert_eq!(m.chapter.as_deref(), Some("第一章"));
        assert_eq!(m.version, 2, "更新后版本号 +1");
        // 旧版本号再更新 → 2004
        let err = dao.update(id, &ok_patch).unwrap_err();
        assert_eq!(err.code, 2004, "版本不一致应报乐观锁冲突");
    }

    #[test]
    fn soft_delete_excluded_from_get_and_list() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let id = dao.insert(1, "images/a.png").unwrap();
        dao.soft_delete(id).unwrap();
        assert_eq!(dao.get(id).unwrap_err().code, 1002);
        assert_eq!(dao.list(&query(1, 20)).unwrap().total, 0);
        // 重复删除 → 1002
        assert_eq!(dao.soft_delete(id).unwrap_err().code, 1002);
    }

    #[test]
    fn tags_crud_and_binding() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let mid = dao.insert(1, "images/a.png").unwrap();
        let t1 = dao.create_tag(1, "易错", Some("#f00")).unwrap();
        // 同名重复创建返回同一 ID（幂等）
        let t1_again = dao.create_tag(1, "易错", None).unwrap();
        assert_eq!(t1, t1_again);
        let t2 = dao.create_tag(1, "期中卷", None).unwrap();
        dao.set_mistake_tags(mid, &[t1, t2]).unwrap();
        let names: Vec<String> = dao
            .tags_of_mistake(mid)
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names.len(), 2);
        // 覆盖式重设
        dao.set_mistake_tags(mid, &[t2]).unwrap();
        assert_eq!(dao.tags_of_mistake(mid).unwrap().len(), 1);
        // 列表摘要回填标签
        let page = dao.list(&query(1, 20)).unwrap();
        assert_eq!(page.items[0].tags, vec!["期中卷".to_string()]);
        // 按标签筛选
        let mut q = query(1, 20);
        q.tag_id = Some(t1);
        assert_eq!(dao.list(&q).unwrap().total, 0);
        q.tag_id = Some(t2);
        assert_eq!(dao.list(&q).unwrap().total, 1);
    }

    #[test]
    fn counts_for_home() {
        let conn = dao_fixture();
        let dao = MistakeDao::new(&conn);
        let a = dao.insert(1, "images/a.png").unwrap();
        dao.insert(1, "images/b.png").unwrap();
        dao.update_status(a, "conflict").unwrap();
        let (pending, conflict, total) = dao.counts_for_home(1).unwrap();
        assert_eq!((pending, conflict, total), (1, 1, 2));
    }
}
