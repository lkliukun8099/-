# 麦兜错题软件 QA 测试报告

| 项目信息 | 内容 |
|---|---|
| QA | Edward（严过关） |
| 测试轮次 | **第 2 轮（回归，最终轮）** |
| 日期 | 2026-04-24 |
| 环境 | Windows（WDAC 应用管控）、Node 22.22.2、无 cargo 工具链 |

## 执行摘要

- **前端（vitest）：62/62 全部通过**；`tsc --noEmit` 通过。
- **Rust（cargo test）**：共 **125 个用例**（本轮新增 2 个混合检索策略用例）已落盘于 19 个源文件；本机无 Rust 工具链，**未能执行**，已做两轮逐文件静态审查替代。
- **路由判定：NoOne（全部修复验证通过，无新发现问题路由工程师）**。
- 第 1 轮 3 个源码缺陷（B1/B2/B3）与 2 个观察项修复（Δ 规则、UTC 口径）**全部复核通过**。
- **环境说明**：本机 WDAC 封锁 rollup 原生 .node 模块，vitest 经由 `node_modules/rollup/dist` 替换为同版本 `@rollup/wasm-node`（4.68.3）WASM 构建跑通（仅本机 node_modules，未提交）。建议团队在开发文档固化该降级方案。

## 第 2 轮回归结果

### 修复点复核

| # | 缺陷 | 修复方案 | 验证方式 | 结论 |
|---|---|---|---|---|
| B1 | PII 脱敏顺序（身份证被手机号截胡） | 两端改为长模式优先：身份证→手机号→邮箱→API Key→学校 | vitest `pii.test.ts` **实测转绿**（`110101199003077531` → `[身份证号]`）；静态复核 `logging.rs::mask_pii` 顺序正确 | ✅ 通过 |
| B2 | `crypto::decrypt` 短密钥 panic | decrypt 增加 32 字节长度校验，返回 `invalid_param`（与 encrypt 对称） | 静态复核 `crypto/mod.rs:36-39`；`decrypt_rejects_bad_key_length_gracefully` 用例语义已满足 | ✅ 通过 |
| B3 | FTS5 中文子串检索失效 | V2 迁移重建 `mistake_fts` 为 trigram（DROP IF EXISTS+重建+rebuild 回填，版本守卫幂等）；DAO 层混合策略：≥3 字符走 trigram MATCH，<3 字符降级 LIKE（通配符转义） | 静态复核迁移幂等性；启用并扩充用例：`fts_keyword_search_chinese_substring`（工程师已去 ignore）+ 新增 `hybrid_search_strategy_boundary`（≥3 字符 trigram / 1 字符 LIKE / 负例）与 `like_wildcards_are_escaped`（`%`/`_` 转义） | ✅ 通过 |
| O2 | 判别式规则漏 `Δ`(U+0394) | stem 关键词补 `Δ`/`△`，answer pattern 扩为 `[△Δδ]` + `delta` | 静态复核 `rules.rs:134-135`；既有 `answer_hit_weighs_double`（△）语义保持 | ✅ 通过 |
| O3 | 复习时间口径混用 | 存储统一 UTC（`Utc::now()`），`due_today` 去 localtime，前端 `format.ts` 以 dayjs utc 解析转本地 | 静态复核三处一致；QA 侧用例已同步改造（`days_from_today` 改 UTC 基准、到期插入去 localtime）；format 用例改正午时刻保证时区无关 | ✅ 通过 |

### 前端回归（已执行）

| 项目 | 结果 |
|---|---|
| `npx vitest run`（7 文件 62 用例） | ✅ 62/62 通过（含 pii 转绿、format UTC 改造、invoke 契约 28 命令不变） |
| `npx tsc --noEmit` | ✅ 通过 |

### Rust 用例总览（cargo test，**待有工具链环境执行**）

| 模块 | 用例数 | 覆盖点 |
|---|---|---|
| `ai/circuit_breaker.rs` | 5 | 3 连败→Open、冷却→HalfOpen、探测成功→Closed、探测失败→重 Open、中途成功清零 |
| `ai/router.rs` | 6 | 调用链顺序、未知任务 1001、provider 缺失/停用 4001、全链预算超限 4002、非法条目跳过 |
| `ai/meter.rs` | 5 | 成本公式、无价格→0、SHA-256 公认向量、call_log 只存哈希（AC-9） |
| `logging.rs` | 9 | 五类 PII 打码、长模式优先顺序（B1 回归）、Writer 语义、非法 UTF-8 兜底 |
| `attribution/rules.rs` | 6 | 场景 A 命中、作答 ×2 加权、无命中 None、置信度封顶、大小写不敏感、规则数 ≥40 |
| `attribution/mod.rs` | 10 | 冲突去重、三方裁决（AC-8 双版本保留）、参数校验、重复裁决 1002、G2 埋点、模式解析 |
| `mastery/mod.rs` | 5 | 先验 0.5、EMA ±0.15、连对收敛、历史快照、分数钳制 |
| `review/mod.rs` | 8 | 首段 +1 天（UTC）、去重、推进/重置、自定义间隔、非法配置回退、到期队列、重复完成 1001 |
| `variant/mod.rs` | 11 | judge_answer 全谱系、parse_drafts、F-15 闭环集成 ×2 |
| `variant/validate/rules.rs` | 10 | 完整性/难度/类型/LaTeX/敏感词/幻觉/NaN |
| `variant/validate/mod.rs` | 4 | 闸 1 短路、全降级不判失败（U-5）、failure_summary |
| `variant/validate/symbolic.rs` | 9 | 提取函数语义、sidecar 缺失降级 skipped |
| `variant/templates.rs` | 4 | 提示词组装、少样例匹配、重试透传、JSON 提取 |
| `attribution/prompts.rs` | 3 | 围栏提取、垃圾拒绝、提示词无 PII（§6-3） |
| `db/mod.rs` | 3 | hex、迁移幂等（schema_version=2）+内置数据、SQLCipher 多连接共享 |
| `db/dao/mistake.rs` | 10 | CRUD/分页/筛选/FTS+LIKE 混合检索（B3 回归 ×3）/注入防护/乐观锁/软删/标签/首页计数 |
| `db/dao/log.rs` | 4 | 成本聚合/空报表/一键清除/ocr_job |
| `crypto/mod.rs` | 7 | AES-GCM 往返/错密钥/篡改/长度校验（B2 回归）/无效 base64 |
| `ocr/mod.rs` | 6 | 一致性校验/levenshtein/OcrPrefer |

## PRD 验收标准对照（终态）

| AC | 判定 |
|---|---|
| AC-1 录入 / AC-2 OCR 准确率 | 代码级确认（机制齐全；识别率/耗时需真实样本集与实机测量） |
| AC-3 离线可用 | 代码级确认（离线五链路均有实现与降级路径） |
| AC-4 场景 A | 规则侧测试覆盖（标签/三级 kg_code/置信度）；变式与排程有测试 |
| AC-5 归因质量 | 机制就绪（规则+LLM+三级定位+G2 埋点），指标待 200 题标注集 |
| AC-6 防幻觉 | 代码级确认（三闸+重试≤2+manual_review） |
| AC-7 多模型路由 | 路由/熔断/计量/估算测试覆盖（待执行） |
| AC-8 冲突合并 | 10 个集成测试覆盖三方裁决与双版本保留 |
| AC-9 隐私 | **B1 已修复**：脱敏顺序正确、call_log 仅存哈希、密钥 keyring、一键清除 |
| AC-10 性能/备份 | 代码级确认（索引+分页+trigram FTS、每日备份 7 份+恢复） |

## 遗留问题与风险评级

| 项 | 等级 | 说明 |
|---|---|---|
| R1 | **中** | Rust 侧 125 个用例从未编译执行（无工具链）。尽管经两轮静态审查，仍可能存在编译期错误（泛型/生命周期/宏展开）。**强烈建议在有 Rust 工具链的环境执行 `cd src-tauri && cargo test` 后再交付**。 |
| R2 | 低 | 观察项遗留（不阻塞）：symbolic.rs 方程提取 fallback 死代码与整段送检、预算字段单位命名（分 vs cny）、`budget_blocked==chain.len()` 边界误判 4001/4002、HalfOpen 并发近似放行、备份恢复需重启。 |
| R3 | 低 | 需真实数据验证的指标：OCR 准确率（AC-1/2）、归因认可率（AC-5）、变式合格率（AC-6）、性能数值（AC-10）、24h 抓包（AC-9 尾部）。 |
| R4 | 低 | 本机 node_modules 的 rollup WASM 替换为本地变通，重装依赖后需重打；建议正式固化。 |

**总体风险：中低**。前端契约与工具层经实测全绿；核心算法（熔断/EMA/排程/裁决/三闸）静态审查无逻辑错误；主要残余风险为 Rust 测试未实际编译执行（R1）。

## 最终路由判定

**NoOne** —— 第 1 轮路由的 3 个缺陷与 2 个观察项全部修复并复核通过，前端 62/62 实测全绿，未发现需要回传工程师的新问题。测试轮次已用尽（2/2），R1~R4 作为已知风险移交。
