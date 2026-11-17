//! 数据层：SQLCipher 连接池 + 密钥派生。
//!
//! 设计要点（架构 §3.3-1）：
//! - 打开连接后首条语句为 `PRAGMA key`；
//! - 数据库密钥为首次启动随机生成的 256-bit 值（hex），存 OS 钥匙串，不落盘；
//! - 预留 `derive_key_from_passphrase`：用户设置口令后可经 Argon2 派生替换随机密钥。

pub mod dao;
pub mod migrate;

use std::path::PathBuf;
use std::sync::Arc;

use rand::RngCore;
use rusqlite::Connection;

use crate::error::AppError;

/// 钥匙串中 DB 密钥的服务名/条目名
const KEYRING_SERVICE: &str = "maiduo-cuotiben";
const KEYRING_DB_KEY: &str = "db-key-v1";

/// 连接池（按连接创建，内部共享路径与密钥）。
///
/// rusqlite `Connection` 非 `Sync`，因此池的语义是"按需开新连接"，
/// SQLite WAL 模式下多连接并发读写足够支撑桌面端负载。
#[derive(Clone)]
pub struct DbPool {
    inner: Arc<DbPoolInner>,
}

struct DbPoolInner {
    path: PathBuf,
    key: String,
}

impl DbPool {
    /// 初始化：取/建密钥 → 打开库 → 执行迁移。
    pub fn init(path: PathBuf) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let key = load_or_create_db_key()?;
        let pool = Self {
            inner: Arc::new(DbPoolInner { path, key }),
        };
        let conn = pool.conn()?;
        migrate::run_migrations(&conn)?;
        Ok(pool)
    }

    /// 打开一个新连接并完成 SQLCipher 解锁与基础 PRAGMA。
    pub fn conn(&self) -> Result<Connection, AppError> {
        let conn = Connection::open(&self.inner.path)?;
        // 密钥为 hex 字符串，仅含 [0-9a-f]，直接拼接安全
        conn.execute_batch(&format!(
            "PRAGMA key = \"{}\";
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA journal_mode = WAL;",
            self.inner.key
        ))?;
        Ok(conn)
    }

    /// 数据库文件路径（备份用）
    pub fn path(&self) -> &PathBuf {
        &self.inner.path
    }

    /// 内存库（单元测试用）：每个测试独立建连并套用完整 schema。
    #[cfg(test)]
    pub fn memory_conn() -> Result<Connection, AppError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrate::run_migrations(&conn)?;
        Ok(conn)
    }

    /// 测试用临时文件库（测试专用）。
    ///
    /// 注意不能用 `:memory:`：`conn()` 每次新开连接，内存库在多连接下
    /// 各自独立、互相不可见，会破坏 service 层（多次取连接）语义。
    /// 每个测试用唯一临时目录，互不干扰。
    #[cfg(test)]
    pub fn test_pool(tag: &str) -> Result<Self, AppError> {
        let dir = std::env::temp_dir()
            .join(format!("maiduo-test-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        let pool = Self {
            inner: Arc::new(DbPoolInner {
                path: dir.join("test.db"),
                // 固定测试密钥（仅测试环境，真实库密钥来自钥匙串随机生成）
                key: hex_encode(&[0x42u8; 32]),
            }),
        };
        let conn = pool.conn()?;
        migrate::run_migrations(&conn)?;
        Ok(pool)
    }
}

/// 从 OS 钥匙串读取 DB 密钥；不存在则随机生成并写入。
fn load_or_create_db_key() -> Result<String, AppError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_DB_KEY)
        .map_err(|e| AppError::keychain(format!("钥匙串条目创建失败: {e}")))?;
    match entry.get_password() {
        Ok(key) => Ok(key),
        Err(keyring::Error::NoEntry) => {
            // 首次启动：生成 256-bit 随机密钥（hex 64 字符）
            let mut buf = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut buf);
            let key = hex_encode(&buf);
            entry
                .set_password(&key)
                .map_err(|e| AppError::keychain(format!("DB 密钥写入钥匙串失败: {e}")))?;
            tracing::info!("已生成新的数据库密钥并存入 OS 钥匙串");
            Ok(key)
        }
        Err(e) => Err(AppError::keychain(format!("DB 密钥读取失败: {e}"))),
    }
}

/// Argon2id 口令派生（预留：用户设置自定义口令时替换随机密钥）。
pub fn derive_key_from_passphrase(passphrase: &str, salt: &[u8]) -> Result<String, AppError> {
    use argon2::Argon2;
    let mut out = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut out)
        .map_err(|e| AppError::internal(format!("Argon2 派生失败: {e}")))?;
    Ok(hex_encode(&out))
}

/// 简单 hex 编码（避免额外引入 hex crate）。
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encode_basic() {
        assert_eq!(hex_encode(&[0x00, 0xff, 0x1a]), "00ff1a");
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn migrations_apply_and_are_idempotent() {
        // 内存库套用完整 schema
        let conn = DbPool::memory_conn().expect("内存库迁移应成功");
        // 重复执行迁移不报错（幂等）
        migrate::run_migrations(&conn).expect("重复迁移应幂等");
        // schema_version 已记录
        let v: String = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'schema_version'",
                [],
                |r| r.get(0),
            )
            .expect("schema_version 应存在");
        assert_eq!(v, "2", "当前迁移序列 V1+V2（trigram FTS）");
        // 内置数据：默认档案 + 四学科
        let profiles: i64 = conn
            .query_row("SELECT COUNT(*) FROM profile", [], |r| r.get(0))
            .unwrap();
        assert_eq!(profiles, 1);
        let subjects: i64 = conn
            .query_row("SELECT COUNT(*) FROM subject", [], |r| r.get(0))
            .unwrap();
        assert_eq!(subjects, 4);
        // 默认路由策略已写入（5 类任务）
        let policies: i64 = conn
            .query_row("SELECT COUNT(*) FROM route_policy", [], |r| r.get(0))
            .unwrap();
        assert_eq!(policies, 5);
    }

    #[test]
    fn test_pool_opens_encrypted_file_db() {
        let pool = DbPool::test_pool("dbinit").expect("测试库应创建成功");
        // 新连接可读写（SQLCipher 密钥在池内共享）
        let conn = pool.conn().unwrap();
        conn.execute("INSERT INTO tag(profile_id, name) VALUES (1, 'qa')", [])
            .unwrap();
        let conn2 = pool.conn().unwrap();
        let n: i64 = conn2
            .query_row("SELECT COUNT(*) FROM tag WHERE name = 'qa'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "多连接应共享同一加密库文件");
    }
}
