//! 变式题 DAO。

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// 变式题完整行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantQuestion {
    pub id: i64,
    pub mistake_id: i64,
    pub kg_node_id: Option<i64>,
    pub content: String,
    pub content_latex: Option<String>,
    pub answer: String,
    pub solution: Option<String>,
    pub difficulty: i64,
    pub variant_type: String,
    pub generator_model: Option<String>,
    pub validate_status: String,
    pub validate_detail: Option<String>,
    pub created_at: String,
}

/// 流式生成过程中的草稿（与 TS `VariantDraft` 镜像，架构 §4.4）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantDraft {
    pub content: String,
    pub content_latex: Option<String>,
    pub answer: String,
    pub solution: Option<String>,
    pub difficulty: i64,
    pub variant_type: String,
    pub validate_status: String,
    /// 已入库后的 ID（流式后半段回填）
    pub variant_id: Option<i64>,
}

pub struct VariantDao<'a> {
    conn: &'a Connection,
}

impl<'a> VariantDao<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_variant(row: &Row) -> rusqlite::Result<VariantQuestion> {
        Ok(VariantQuestion {
            id: row.get("id")?,
            mistake_id: row.get("mistake_id")?,
            kg_node_id: row.get("kg_node_id")?,
            content: row.get("content")?,
            content_latex: row.get("content_latex")?,
            answer: row.get("answer")?,
            solution: row.get("solution")?,
            difficulty: row.get("difficulty")?,
            variant_type: row.get("variant_type")?,
            generator_model: row.get("generator_model")?,
            validate_status: row.get("validate_status")?,
            validate_detail: row.get("validate_detail")?,
            created_at: row.get("created_at")?,
        })
    }

    /// 插入变式题，返回 ID。
    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &self,
        mistake_id: i64,
        kg_node_id: Option<i64>,
        draft: &VariantDraft,
        generator_model: Option<&str>,
        validate_detail: Option<&str>,
    ) -> Result<i64, AppError> {
        self.conn.execute(
            "INSERT INTO variant_question(mistake_id, kg_node_id, content, content_latex,
               answer, solution, difficulty, variant_type, generator_model,
               validate_status, validate_detail)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                mistake_id,
                kg_node_id,
                draft.content,
                draft.content_latex,
                draft.answer,
                draft.solution,
                draft.difficulty,
                draft.variant_type,
                generator_model,
                draft.validate_status,
                validate_detail,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 按 ID 取变式题。
    pub fn get(&self, id: i64) -> Result<VariantQuestion, AppError> {
        self.conn
            .query_row(
                "SELECT * FROM variant_question WHERE id = ?1",
                params![id],
                Self::row_to_variant,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::not_found(format!("变式题 #{id} 不存在"))
                }
                other => other.into(),
            })
    }

    /// 某错题的全部变式题。
    pub fn list_for_mistake(&self, mistake_id: i64) -> Result<Vec<VariantQuestion>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM variant_question WHERE mistake_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![mistake_id], Self::row_to_variant)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 更新校验结果（重生成后覆盖）。
    pub fn update_validate(
        &self,
        id: i64,
        status: &str,
        detail: Option<&str>,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "UPDATE variant_question SET validate_status = ?2, validate_detail = ?3 WHERE id = ?1",
            params![id, status, detail],
        )?;
        Ok(())
    }

    /// 可训练的变式题（校验通过），今日训练用。
    pub fn list_trainable(&self, profile_id: i64, limit: i64) -> Result<Vec<VariantQuestion>, AppError> {
        let mut stmt = self.conn.prepare(
            "SELECT v.* FROM variant_question v
             JOIN mistake m ON m.id = v.mistake_id
             WHERE m.profile_id = ?1 AND m.deleted = 0 AND v.validate_status = 'passed'
             ORDER BY v.created_at DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![profile_id, limit], Self::row_to_variant)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}
