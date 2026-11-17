//! 知识图谱服务：三级树查询、搜索、路径组装。

pub mod seed_loader;

use crate::db::dao::kg::{KgDao, KgNode, KgTree};
use crate::db::DbPool;
use crate::error::AppError;

/// 学科三级图谱树。
pub fn kg_tree(db: &DbPool, subject_id: i64) -> Result<KgTree, AppError> {
    let conn = db.conn()?;
    KgDao::new(&conn).tree(subject_id)
}

/// 图谱搜索（名称/编码模糊）。
pub fn kg_search(db: &DbPool, subject_id: i64, keyword: &str) -> Result<Vec<KgNode>, AppError> {
    if keyword.trim().is_empty() {
        return Err(AppError::invalid_param("搜索关键词不能为空"));
    }
    let conn = db.conn()?;
    KgDao::new(&conn).search(subject_id, keyword.trim())
}

/// 节点路径（根→叶名称数组），归因展示与 TS `kg_path` 镜像。
pub fn node_path(conn: &rusqlite::Connection, node_id: i64) -> Vec<String> {
    KgDao::new(conn).path_names(node_id).unwrap_or_default()
}
