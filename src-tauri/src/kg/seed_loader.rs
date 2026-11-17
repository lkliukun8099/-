//! 知识图谱种子数据导入（首版：苏教版数学三级图谱，约 200 节点）。
//!
//! 种子文件随包分发：`resources/kg_seed/math_sujiao.json`，首次启动导入，
//! 幂等标记存于 `app_meta.kg_seeded_math_sujiao`。

use serde::Deserialize;
use tauri::Manager;

use crate::db::dao::kg::KgDao;
use crate::db::DbPool;
use crate::error::AppError;

/// 种子文件结构。
#[derive(Debug, Deserialize)]
pub struct SeedFile {
    pub subject: SeedSubject,
    pub nodes: Vec<SeedNode>,
    #[serde(default)]
    pub edges: Vec<SeedEdge>,
}

#[derive(Debug, Deserialize)]
pub struct SeedSubject {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct SeedNode {
    pub code: String,
    pub name: String,
    pub level: i64,
    #[serde(default)]
    pub children: Vec<SeedNode>,
}

#[derive(Debug, Deserialize)]
pub struct SeedEdge {
    pub from: String,
    pub to: String,
    #[serde(default = "default_relation")]
    pub relation: String,
}

fn default_relation() -> String {
    "prerequisite".to_string()
}

/// 确保种子数据已导入（幂等）。
pub fn ensure_seeded(db: &DbPool, app: &tauri::App) -> Result<(), AppError> {
    let conn = db.conn()?;

    let seeded: bool = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'kg_seeded_math_sujiao'",
            [],
            |r| r.get::<_, String>(0),
        )
        .map(|v| v == "1")
        .unwrap_or(false);
    if seeded {
        return Ok(());
    }

    // 解析随包资源路径
    let resource = app
        .path()
        .resolve("resources/kg_seed/math_sujiao.json", tauri::path::BaseDirectory::Resource);

    let path = match resource {
        Ok(p) if p.exists() => p,
        _ => {
            // 开发环境回退：相对 src-tauri 目录
            let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources/kg_seed/math_sujiao.json");
            if !dev_path.exists() {
                tracing::warn!("图谱种子文件缺失，跳过导入: {}", dev_path.display());
                return Ok(());
            }
            dev_path
        }
    };

    let raw = std::fs::read_to_string(&path)?;
    let seed: SeedFile = serde_json::from_str(&raw)?;
    import_seed(&conn, &seed)?;

    conn.execute(
        "INSERT INTO app_meta(key, value) VALUES('kg_seeded_math_sujiao', '1')
         ON CONFLICT(key) DO UPDATE SET value = '1'",
        [],
    )?;
    tracing::info!("苏教版数学图谱种子导入完成");
    Ok(())
}

/// 事务内导入：节点（递归）+ 边。
fn import_seed(conn: &rusqlite::Connection, seed: &SeedFile) -> Result<(), AppError> {
    let dao = KgDao::new(conn);
    let subject_id = dao
        .subject_id_by_code(&seed.subject.code)?
        .ok_or_else(|| AppError::internal(format!("学科 {} 未初始化", seed.subject.code)))?;

    let tx = conn.unchecked_transaction()?;
    let tx_dao = KgDao::new(&tx);

    // 递归插入节点，记录 code -> id 映射供边解析
    let mut code_to_id: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for root in &seed.nodes {
        insert_node_recursive(&tx_dao, subject_id, None, root, &mut code_to_id)?;
    }

    // 插入边（两端编码均需已入库）
    for edge in &seed.edges {
        let from = code_to_id.get(&edge.from);
        let to = code_to_id.get(&edge.to);
        if let (Some(f), Some(t)) = (from, to) {
            tx_dao.insert_edge(subject_id, *f, *t, &edge.relation)?;
        }
    }

    tx.commit()?;
    Ok(())
}

fn insert_node_recursive(
    dao: &KgDao,
    subject_id: i64,
    parent_id: Option<i64>,
    node: &SeedNode,
    code_to_id: &mut std::collections::HashMap<String, i64>,
) -> Result<(), AppError> {
    let id = dao.insert_node(
        subject_id,
        parent_id,
        &node.code,
        &node.name,
        node.level,
        "sujiao",
    )?;
    code_to_id.insert(node.code.clone(), id);
    for child in &node.children {
        insert_node_recursive(dao, subject_id, Some(id), child, code_to_id)?;
    }
    Ok(())
}
