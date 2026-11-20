//! 版本化迁移器：按版本号顺序执行 SQL，版本记录在 `app_meta.schema_version`。

use rusqlite::Connection;

use crate::error::AppError;

/// 全部迁移脚本（版本号从 1 开始递增，禁止修改历史版本内容）。
const MIGRATIONS: &[(u32, &str)] = &[(1, V1_INIT), (2, V2_FTS_TRIGRAM)];

/// 执行所有未应用的迁移（每个版本一个事务）。
pub fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    // 元信息表先于版本判断存在
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_meta (key TEXT PRIMARY KEY, value TEXT);",
    )?;

    let current: u32 = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        tracing::info!("应用数据库迁移 v{version}");
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO app_meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [version.to_string()],
        )?;
        tx.commit()?;
        tracing::info!("迁移 v{version} 完成");
    }
    Ok(())
}

/// V1：完整初始 schema（架构文档 §3.2）。
const V1_INIT: &str = r#"
-- 元信息与多档案（F-22 预留，首版默认单档案 id=1）
CREATE TABLE profile (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL DEFAULT '默认档案',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE subject (
  id INTEGER PRIMARY KEY,
  code TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL
);

-- 知识图谱（F-06/F-07）
CREATE TABLE kg_node (
  id INTEGER PRIMARY KEY,
  subject_id INTEGER NOT NULL REFERENCES subject(id),
  parent_id INTEGER REFERENCES kg_node(id),
  code TEXT NOT NULL,
  name TEXT NOT NULL,
  level INTEGER NOT NULL CHECK(level BETWEEN 1 AND 3),
  textbook TEXT NOT NULL DEFAULT 'sujiao',
  extra TEXT,
  UNIQUE(subject_id, code)
);
CREATE TABLE kg_edge (
  id INTEGER PRIMARY KEY,
  subject_id INTEGER NOT NULL,
  from_node INTEGER NOT NULL REFERENCES kg_node(id),
  to_node INTEGER NOT NULL REFERENCES kg_node(id),
  relation TEXT NOT NULL DEFAULT 'prerequisite'
);
CREATE INDEX idx_kg_node_parent ON kg_node(parent_id);

-- 错题主表
CREATE TABLE mistake (
  id INTEGER PRIMARY KEY,
  profile_id INTEGER NOT NULL REFERENCES profile(id),
  subject_id INTEGER REFERENCES subject(id),
  source_image TEXT NOT NULL,
  processed_image TEXT,
  ocr_text TEXT,
  ocr_latex TEXT,
  ocr_engine TEXT,
  ocr_confidence REAL,
  student_answer TEXT,
  correct_answer TEXT,
  chapter TEXT,
  question_type TEXT,
  difficulty INTEGER DEFAULT 3 CHECK(difficulty BETWEEN 1 AND 5),
  source TEXT,
  status TEXT NOT NULL DEFAULT 'pending_analysis'
         CHECK(status IN ('pending_analysis','analyzing','analyzed','conflict','archived')),
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now')),
  deleted INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_mistake_status ON mistake(status, deleted);
CREATE INDEX idx_mistake_subject ON mistake(subject_id, created_at DESC);
CREATE VIRTUAL TABLE mistake_fts USING fts5(ocr_text, content='mistake', content_rowid='id');

-- FTS 同步触发器
CREATE TRIGGER mistake_fts_insert AFTER INSERT ON mistake BEGIN
  INSERT INTO mistake_fts(rowid, ocr_text) VALUES (new.id, COALESCE(new.ocr_text, ''));
END;
CREATE TRIGGER mistake_fts_delete AFTER DELETE ON mistake BEGIN
  INSERT INTO mistake_fts(mistake_fts, rowid, ocr_text) VALUES('delete', old.id, COALESCE(old.ocr_text, ''));
END;
CREATE TRIGGER mistake_fts_update AFTER UPDATE OF ocr_text ON mistake BEGIN
  INSERT INTO mistake_fts(mistake_fts, rowid, ocr_text) VALUES('delete', old.id, COALESCE(old.ocr_text, ''));
  INSERT INTO mistake_fts(rowid, ocr_text) VALUES (new.id, COALESCE(new.ocr_text, ''));
END;

-- 自定义标签（F-05）
CREATE TABLE tag (id INTEGER PRIMARY KEY, profile_id INTEGER NOT NULL,
                  name TEXT NOT NULL, color TEXT, UNIQUE(profile_id, name));
CREATE TABLE mistake_tag_rel (
  mistake_id INTEGER NOT NULL REFERENCES mistake(id),
  tag_id INTEGER NOT NULL REFERENCES tag(id),
  PRIMARY KEY(mistake_id, tag_id));

-- 错因归因（F-06）：多版本保留
CREATE TABLE attribution (
  id INTEGER PRIMARY KEY,
  mistake_id INTEGER NOT NULL REFERENCES mistake(id),
  origin TEXT NOT NULL CHECK(origin IN ('local','cloud','user')),
  engine TEXT NOT NULL,
  cause_label TEXT NOT NULL,
  cause_detail TEXT,
  kg_node_id INTEGER REFERENCES kg_node(id),
  confidence REAL NOT NULL DEFAULT 0,
  is_active INTEGER NOT NULL DEFAULT 1,
  superseded_by INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_attr_mistake ON attribution(mistake_id, is_active);

-- 冲突表（F-16）
CREATE TABLE conflict (
  id INTEGER PRIMARY KEY,
  entity_type TEXT NOT NULL DEFAULT 'attribution',
  entity_id INTEGER NOT NULL,
  local_attr_id INTEGER NOT NULL REFERENCES attribution(id),
  cloud_attr_id INTEGER NOT NULL REFERENCES attribution(id),
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','resolved','ignored')),
  resolved_choice TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  resolved_at TEXT
);

-- 变式题（F-08/F-09）
CREATE TABLE variant_question (
  id INTEGER PRIMARY KEY,
  mistake_id INTEGER NOT NULL REFERENCES mistake(id),
  kg_node_id INTEGER REFERENCES kg_node(id),
  content TEXT NOT NULL,
  content_latex TEXT,
  answer TEXT NOT NULL,
  solution TEXT,
  difficulty INTEGER NOT NULL DEFAULT 3,
  variant_type TEXT NOT NULL DEFAULT 'isomorphic',
  generator_model TEXT,
  validate_status TEXT NOT NULL DEFAULT 'pending'
         CHECK(validate_status IN ('pending','passed','failed','manual_review')),
  validate_detail TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_variant_mistake ON variant_question(mistake_id, validate_status);

-- 作答记录
CREATE TABLE attempt (
  id INTEGER PRIMARY KEY,
  profile_id INTEGER NOT NULL,
  target_type TEXT NOT NULL CHECK(target_type IN ('mistake','variant')),
  target_id INTEGER NOT NULL,
  answer TEXT,
  is_correct INTEGER NOT NULL,
  duration_ms INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_attempt_target ON attempt(target_type, target_id);

-- 掌握度（F-15）
CREATE TABLE mastery (
  id INTEGER PRIMARY KEY,
  profile_id INTEGER NOT NULL,
  kg_node_id INTEGER NOT NULL REFERENCES kg_node(id),
  score REAL NOT NULL DEFAULT 0,
  attempts INTEGER NOT NULL DEFAULT 0,
  correct INTEGER NOT NULL DEFAULT 0,
  last_assessed_at TEXT,
  UNIQUE(profile_id, kg_node_id)
);
CREATE TABLE mastery_history (
  id INTEGER PRIMARY KEY,
  profile_id INTEGER NOT NULL,
  kg_node_id INTEGER NOT NULL,
  score REAL NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 复习计划（F-14）
CREATE TABLE review_config (
  profile_id INTEGER PRIMARY KEY,
  intervals TEXT NOT NULL DEFAULT '[1,3,7,15,30]'
);
CREATE TABLE review_plan (
  id INTEGER PRIMARY KEY,
  profile_id INTEGER NOT NULL,
  target_type TEXT NOT NULL CHECK(target_type IN ('mistake','variant')),
  target_id INTEGER NOT NULL,
  stage INTEGER NOT NULL DEFAULT 0,
  next_due_at TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','done','skipped','expired')),
  completed_at TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_review_due ON review_plan(profile_id, status, next_due_at);

-- OCR 任务留痕
CREATE TABLE ocr_job (
  id INTEGER PRIMARY KEY,
  mistake_id INTEGER REFERENCES mistake(id),
  engine TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  result_text TEXT, result_latex TEXT, confidence REAL,
  latency_ms INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 模型接入（F-10/F-11/F-19）
CREATE TABLE model_provider (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  protocol TEXT NOT NULL DEFAULT 'openai_compat',
  base_url TEXT NOT NULL,
  key_alias TEXT NOT NULL,
  models TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  priority INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE route_policy (
  id INTEGER PRIMARY KEY,
  task_type TEXT NOT NULL UNIQUE,
  primary_route TEXT NOT NULL,
  fallbacks TEXT NOT NULL DEFAULT '[]',
  timeout_ms INTEGER NOT NULL DEFAULT 30000,
  max_cost_cny REAL,
  monthly_budget_cny REAL
);
CREATE TABLE call_log (
  id INTEGER PRIMARY KEY,
  provider TEXT NOT NULL, model TEXT NOT NULL,
  task_type TEXT NOT NULL,
  tokens_in INTEGER DEFAULT 0, tokens_out INTEGER DEFAULT 0,
  cost_cny REAL DEFAULT 0,
  latency_ms INTEGER, status TEXT NOT NULL,
  error_code TEXT, request_hash TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_calllog_time ON call_log(created_at);

-- 同步预留（F-21）与应用元信息
CREATE TABLE sync_meta (key TEXT PRIMARY KEY, value TEXT);

-- 内置数据：默认档案 + 四学科占位（数学种子由 seed_loader 导入）
INSERT INTO profile(id, name) VALUES (1, '默认档案');
INSERT INTO subject(code, name) VALUES
  ('math', '数学'), ('physics', '物理'), ('chemistry', '化学'), ('english', '英语');

-- 默认路由策略（provider.model 形式，用户可在模型设置中修改）
INSERT INTO route_policy(task_type, primary_route, fallbacks, timeout_ms) VALUES
  ('ocr_attribution', 'deepseek.deepseek-chat', '["glm.glm-4-flash","kimi.moonshot-v1-8k"]', 30000),
  ('variant_gen',     'deepseek.deepseek-chat', '["glm.glm-4-flash"]', 60000),
  ('llm_check',       'glm.glm-4-flash',        '["deepseek.deepseek-chat"]', 30000),
  ('long_text',       'kimi.moonshot-v1-32k',   '["deepseek.deepseek-chat"]', 60000),
  ('vision',          'glm.glm-4v',             '[]', 45000);
"#;

/// V2：FTS5 分词器 unicode61 → trigram（修复中文子串检索失效，F-05）。
///
/// unicode61 不分词中文，整段中文是单 token，中文关键词子串无法命中；
/// trigram 支持 ≥3 字符子串匹配（<3 字符由 DAO 层降级 LIKE 处理）。
/// 迁移幂等：DROP IF EXISTS + 重建 + rebuild 回填存量数据。
const V2_FTS_TRIGRAM: &str = r#"
DROP TRIGGER IF EXISTS mistake_fts_insert;
DROP TRIGGER IF EXISTS mistake_fts_delete;
DROP TRIGGER IF EXISTS mistake_fts_update;
DROP TABLE IF EXISTS mistake_fts;
CREATE VIRTUAL TABLE mistake_fts USING fts5(ocr_text, content='mistake', content_rowid='id', tokenize='trigram');
CREATE TRIGGER mistake_fts_insert AFTER INSERT ON mistake BEGIN
  INSERT INTO mistake_fts(rowid, ocr_text) VALUES (new.id, COALESCE(new.ocr_text, ''));
END;
CREATE TRIGGER mistake_fts_delete AFTER DELETE ON mistake BEGIN
  INSERT INTO mistake_fts(mistake_fts, rowid, ocr_text) VALUES('delete', old.id, COALESCE(old.ocr_text, ''));
END;
CREATE TRIGGER mistake_fts_update AFTER UPDATE OF ocr_text ON mistake BEGIN
  INSERT INTO mistake_fts(mistake_fts, rowid, ocr_text) VALUES('delete', old.id, COALESCE(old.ocr_text, ''));
  INSERT INTO mistake_fts(rowid, ocr_text) VALUES (new.id, COALESCE(new.ocr_text, ''));
END;
-- 回填存量数据
INSERT INTO mistake_fts(mistake_fts) VALUES('rebuild');
"#;
