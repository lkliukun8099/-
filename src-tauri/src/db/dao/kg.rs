//! 知识图谱 DAO：节点/边查询、三级树组装、路径回溯。

use rusqlite::{params, Connection, Row};
use serde::Serialize;

use crate::error::AppError;

/// 图谱节点。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KgNode {
    pub id: i64,
    pub subject_id: i64,
    pub parent_id: Option<i64>,
    pub code: String,
    pub name: String,
    pub level: i64,
    pub textbook: String,
    pub extra: Option<String>,
}

/// 三级树（children 嵌套）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KgTree {
    pub subject_id: i64,
    pub roots: Vec<KgTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KgTreeNode {
    #[serde(flatten)]
    pub node: KgNode,
    pub children: Vec<KgTreeNode>,
}

pub struct KgDao<'a> {
    conn: &'a Connection,
}

impl<'a> KgDao<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    fn row_to_node(row: &Row) -> rusqlite::Result<KgNode> {
        Ok(KgNode {
            id: row.get("id")?,
            subject_id: row.get("subject_id")?,
            parent_id: row.get("parent_id")?,
            code: row.get("code")?,
            name: row.get("name")?,
            level: row.get("level")?,
            textbook: row.get("textbook")?,
            extra: row.get("extra")?,
        })
    }

    /// 学科下全部节点（一次取出后内存组树，万级内无性能问题）。
    pub fn all_for_subject(&self, subject_id: i64) -> Result<Vec<KgNode>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM kg_node WHERE subject_id = ?1 ORDER BY id")?;
        let rows = stmt
            .query_map(params![subject_id], Self::row_to_node)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 组装三级树。
    pub fn tree(&self, subject_id: i64) -> Result<KgTree, AppError> {
        let nodes = self.all_for_subject(subject_id)?;
        // 以 parent_id 分组
        let mut children_map: std::collections::HashMap<Option<i64>, Vec<KgNode>> =
            std::collections::HashMap::new();
        for n in nodes {
            children_map.entry(n.parent_id).or_default().push(n);
        }
        fn build(
            node: KgNode,
            map: &std::collections::HashMap<Option<i64>, Vec<KgNode>>,
        ) -> KgTreeNode {
            let children = map
                .get(&Some(node.id))
                .map(|cs| cs.iter().cloned().map(|c| build(c, map)).collect())
                .unwrap_or_default();
            KgTreeNode { node, children }
        }
        let roots = children_map
            .get(&None)
            .map(|rs| rs.iter().cloned().map(|r| build(r, &children_map)).collect())
            .unwrap_or_default();
        Ok(KgTree { subject_id, roots })
    }

    /// 关键词搜索（名称或编码）。
    pub fn search(&self, subject_id: i64, keyword: &str) -> Result<Vec<KgNode>, AppError> {
        let like = format!("%{}%", keyword.replace('%', ""));
        let mut stmt = self.conn.prepare(
            "SELECT * FROM kg_node WHERE subject_id = ?1 AND (name LIKE ?2 OR code LIKE ?2)
             ORDER BY level, id LIMIT 50",
        )?;
        let rows = stmt
            .query_map(params![subject_id, like], Self::row_to_node)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 按 ID 取节点。
    pub fn get(&self, id: i64) -> Result<Option<KgNode>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM kg_node WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![id], Self::row_to_node)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 按编码取节点（同学科内唯一）。
    pub fn get_by_code(&self, subject_id: i64, code: &str) -> Result<Option<KgNode>, AppError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM kg_node WHERE subject_id = ?1 AND code = ?2")?;
        let mut rows = stmt.query_map(params![subject_id, code], Self::row_to_node)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// 回溯节点路径（根→叶的名称数组），用于归因三级定位展示。
    pub fn path_names(&self, node_id: i64) -> Result<Vec<String>, AppError> {
        let mut path = Vec::new();
        let mut current = Some(node_id);
        // 最多 3 级，防御性上限 8 防环
        for _ in 0..8 {
            match current.and_then(|id| self.get(id).ok().flatten()) {
                Some(node) => {
                    path.push(node.name);
                    current = node.parent_id;
                }
                None => break,
            }
        }
        path.reverse();
        Ok(path)
    }

    /// 插入节点（种子导入用），返回 ID。
    pub fn insert_node(
        &self,
        subject_id: i64,
        parent_id: Option<i64>,
        code: &str,
        name: &str,
        level: i64,
        textbook: &str,
    ) -> Result<i64, AppError> {
        self.conn.execute(
            "INSERT INTO kg_node(subject_id, parent_id, code, name, level, textbook)
             VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(subject_id, code) DO NOTHING",
            params![subject_id, parent_id, code, name, level, textbook],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM kg_node WHERE subject_id = ?1 AND code = ?2",
            params![subject_id, code],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    /// 插入边（种子导入用）。
    pub fn insert_edge(
        &self,
        subject_id: i64,
        from_node: i64,
        to_node: i64,
        relation: &str,
    ) -> Result<(), AppError> {
        self.conn.execute(
            "INSERT INTO kg_edge(subject_id, from_node, to_node, relation) VALUES (?1,?2,?3,?4)",
            params![subject_id, from_node, to_node, relation],
        )?;
        Ok(())
    }

    /// 学科 ID 按编码查询。
    pub fn subject_id_by_code(&self, code: &str) -> Result<Option<i64>, AppError> {
        let mut stmt = self.conn.prepare("SELECT id FROM subject WHERE code = ?1")?;
        let mut rows = stmt.query_map(params![code], |r| r.get(0))?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }
}
