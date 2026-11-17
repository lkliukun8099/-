//! 变式生成编排（F-08/F-09）：模板 + LLM 生成 → 三闸校验 → 重试≤2 次 → 入库推送。

pub mod templates;
pub mod validate;

use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;
use validate::{ValidatePipeline, ValidateReport};

use crate::ai::adapter::{ChatMessage, ChatRequest};
use crate::ai::Gateway;
use crate::db::dao::attribution::AttributionDao;
use crate::db::dao::kg::KgDao;
use crate::db::dao::mastery::MasteryDao;
use crate::db::dao::mistake::MistakeDao;
use crate::db::dao::variant::{VariantDao, VariantDraft};
use crate::db::DbPool;
use crate::error::AppError;
use crate::kg;
use crate::mastery;
use crate::review::ReviewScheduler;

/// 最大重生成次数
const MAX_RETRY: u32 = 2;

/// 变式生成器。
pub struct VariantGenerator {
    db: DbPool,
    gateway: Arc<Gateway>,
    validator: ValidatePipeline,
}

/// 作答反馈（variant_submit_attempt 返回）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptFeedback {
    pub is_correct: bool,
    pub feedback: String,
    pub mastery_delta: Option<f64>,
}

impl VariantGenerator {
    pub fn new(db: DbPool, gateway: Arc<Gateway>) -> Self {
        Self {
            validator: ValidatePipeline::new(gateway.clone()),
            db,
            gateway,
        }
    }

    /// 生成变式题并逐道推入 Channel（含校验状态与入库 ID）。
    pub async fn generate(
        &self,
        mistake_id: i64,
        count: u32,
        channel: Channel<VariantDraft>,
    ) -> Result<(), AppError> {
        let count = count.clamp(1, 5);
        let mistake = {
            let conn = self.db.conn()?;
            MistakeDao::new(&conn).get(mistake_id)?
        };

        // 知识点上下文（来自生效归因）
        let (kg_node_id, kg_code, kg_path) = {
            let conn = self.db.conn()?;
            let active = AttributionDao::new(&conn).active_for_mistake(mistake_id)?;
            match active.and_then(|a| a.kg_node_id) {
                Some(nid) => {
                    let node = KgDao::new(&conn).get(nid)?;
                    (
                        Some(nid),
                        node.map(|n| n.code),
                        kg::node_path(&conn, nid),
                    )
                }
                None => (None, None, Vec::new()),
            }
        };

        // 生成 + 校验 + 重试
        let mut retry = 0;
        let mut failure_reason: Option<String> = None;
        let mut final_drafts: Vec<(VariantDraft, ValidateReport)> = Vec::new();

        while retry <= MAX_RETRY {
            let raw = self
                .call_llm(&mistake, count, &kg_path, kg_code.as_deref(), failure_reason.as_deref())
                .await?;
            let llm_drafts = parse_drafts(&raw)?;
            if llm_drafts.is_empty() {
                return Err(AppError::validate("LLM 未生成任何变式题"));
            }

            let mut round: Vec<(VariantDraft, ValidateReport)> = Vec::new();
            let mut round_failures: Vec<String> = Vec::new();
            for ld in llm_drafts {
                let draft = VariantDraft {
                    content: ld.content,
                    content_latex: ld.content_latex,
                    answer: ld.answer,
                    solution: ld.solution,
                    difficulty: ld.difficulty.clamp(1, 5),
                    variant_type: ld.variant_type,
                    validate_status: "pending".into(),
                    variant_id: None,
                };
                let report = self.validator.validate(&draft).await;
                if report.final_status != "passed" {
                    round_failures.push(ValidatePipeline::failure_summary(&report));
                }
                round.push((draft, report));
            }

            let all_passed = round.iter().all(|(_, r)| r.final_status == "passed");
            final_drafts = round;
            if all_passed {
                break;
            }
            retry += 1;
            if retry <= MAX_RETRY {
                failure_reason = Some(round_failures.join("；"));
                tracing::info!("变式校验未过，第 {retry} 次重生成: {}", failure_reason.as_deref().unwrap_or(""));
            }
        }

        // 入库并逐道推送（仍失败的标记 manual_review）
        let served_model = "variant_gen".to_string();
        for (mut draft, report) in final_drafts {
            draft.validate_status = if report.final_status == "passed" {
                "passed".into()
            } else {
                "manual_review".into()
            };
            let detail = serde_json::to_string(&report).ok();
            let variant_id = {
                let conn = self.db.conn()?;
                VariantDao::new(&conn).insert(
                    mistake_id,
                    kg_node_id,
                    &draft,
                    Some(&served_model),
                    detail.as_deref(),
                )?
            };
            draft.variant_id = Some(variant_id);
            if channel.send(draft).is_err() {
                tracing::warn!("变式推送 Channel 已断开，剩余结果仅入库");
                break;
            }
        }
        Ok(())
    }

    /// 调用 LLM 生成（走 Gateway 非流式，JSON 结构更稳）。
    async fn call_llm(
        &self,
        mistake: &crate::db::dao::mistake::Mistake,
        count: u32,
        kg_path: &[String],
        kg_code: Option<&str>,
        extra_instruction: Option<&str>,
    ) -> Result<String, AppError> {
        let prompt = templates::build_generate_prompt(
            mistake,
            count,
            kg_path,
            kg_code,
            extra_instruction,
        );
        let req = ChatRequest {
            model: String::new(),
            messages: vec![
                ChatMessage::system(templates::VARIANT_SYSTEM),
                ChatMessage::user(prompt),
            ],
            temperature: Some(0.7),
            max_tokens: Some(4000),
        };
        let resp = self.gateway.chat_once("variant_gen", req).await?;
        Ok(resp.content)
    }

    /// 变式作答反馈闭环（F-15）：判分 → attempt 落库 → 掌握度 EMA → 原题排复习。
    pub fn submit_attempt(
        &self,
        variant_id: i64,
        answer: &str,
        duration_ms: Option<i64>,
        profile_id: i64,
    ) -> Result<AttemptFeedback, AppError> {
        let variant = {
            let conn = self.db.conn()?;
            VariantDao::new(&conn).get(variant_id)?
        };
        let is_correct = judge_answer(answer, &variant.answer);

        // 作答落库
        {
            let conn = self.db.conn()?;
            MasteryDao::new(&conn).insert_attempt(
                profile_id,
                "variant",
                variant_id,
                Some(answer),
                is_correct,
                duration_ms,
            )?;
        }

        // 掌握度更新
        let mastery_delta = match variant.kg_node_id {
            Some(nid) => Some(mastery::update_after_attempt(
                &self.db,
                profile_id,
                nid,
                is_correct,
            )?),
            None => None,
        };

        // 原错题排入复习队列（1/3/7/15/30）
        ReviewScheduler::new(self.db.clone()).schedule(
            profile_id,
            "mistake",
            variant.mistake_id,
        )?;

        let feedback = if is_correct {
            "回答正确！该知识点掌握度已提升，原题已排入复习队列。".to_string()
        } else {
            format!(
                "答案不正确。参考答案：{}\n解析：{}",
                variant.answer,
                variant.solution.clone().unwrap_or_else(|| "（暂无解析）".into())
            )
        };
        Ok(AttemptFeedback {
            is_correct,
            feedback,
            mastery_delta,
        })
    }
}

/// 解析 LLM 输出为草稿列表。
fn parse_drafts(raw: &str) -> Result<Vec<templates::LlmVariantDraft>, AppError> {
    let json_str = templates::extract_json_array(raw)?;
    let drafts: Vec<templates::LlmVariantDraft> = serde_json::from_str(json_str)
        .map_err(|e| AppError::validate(format!("变式 JSON 解析失败: {e}")))?;
    Ok(drafts)
}

/// 答案判等：归一化后字符串比对 + 数值容差 + 集合无序比对。
pub fn judge_answer(student: &str, correct: &str) -> bool {
    let s = normalize_answer(student);
    let c = normalize_answer(correct);
    if s.is_empty() || c.is_empty() {
        return false;
    }
    if s == c {
        return true;
    }
    // 数值容差
    if let (Ok(a), Ok(b)) = (s.parse::<f64>(), c.parse::<f64>()) {
        return (a - b).abs() < 1e-6;
    }
    // 集合/多值无序比对（逗号、顿号分隔）
    let s_set = split_values(&s);
    let c_set = split_values(&c);
    if s_set.len() > 1 && s_set.len() == c_set.len() {
        let mut s_sorted = s_set;
        let mut c_sorted = c_set;
        s_sorted.sort();
        c_sorted.sort();
        return s_sorted == c_sorted;
    }
    false
}

/// 答案归一化：去空白/全角转半角/统一运算符/去答案前缀与尾部标点。
fn normalize_answer(input: &str) -> String {
    let mut s = input.trim().to_string();
    for prefix in ["答案：", "答案:", "答：", "答:", "解：", "解:"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
        }
    }
    s.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '，' | '、' => ',',
            '。' => '.',
            '（' => '(',
            '）' => ')',
            '×' => '*',
            '÷' => '/',
            '＝' => '=',
            '－' => '-',
            '＋' => '+',
            other => other.to_ascii_lowercase(),
        })
        .collect::<String>()
        .trim_end_matches(['.', ','])
        .to_string()
}

/// 多值切分。
fn split_values(s: &str) -> Vec<String> {
    s.split(',')
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judge_exact_match() {
        assert!(judge_answer("a≥2", "a≥2"));
        assert!(!judge_answer("a≥1", "a≥2"));
    }

    #[test]
    fn judge_numeric_tolerance_and_format() {
        assert!(judge_answer("3.0", "3"));
        assert!(judge_answer("  3 ", "3"));
        assert!(judge_answer("3.0000001", "3"));
        assert!(!judge_answer("3.1", "3"));
    }

    #[test]
    fn judge_normalizes_prefix_and_fullwidth() {
        assert!(judge_answer("答案：x=2", "x=2"));
        assert!(judge_answer("答:x=2", "x=2"));
        assert!(judge_answer("（1，2）", "(1,2)"), "全角括号/逗号应归一");
        assert!(judge_answer("X=2", "x=2"), "大小写不敏感");
    }

    #[test]
    fn judge_set_order_insensitive() {
        assert!(judge_answer("1，2", "2，1"), "多值答案与顺序无关");
        assert!(judge_answer("1、2", "2,1"), "顿号等价于逗号");
        assert!(!judge_answer("1,2", "1,3"));
    }

    #[test]
    fn judge_empty_always_wrong() {
        assert!(!judge_answer("", "3"));
        assert!(!judge_answer("3", ""));
    }

    #[test]
    fn judge_operator_normalization() {
        assert!(judge_answer("2×3", "2*3"));
        assert!(judge_answer("6÷2", "6/2"));
    }

    #[test]
    fn parse_drafts_tolerates_code_fence() {
        let raw = "好的，以下是变式题：\n```json\n[{\"content\":\"计算 1+1 的结果并说明\",\"answer\":\"2\",\"solution\":null,\"difficulty\":1,\"variant_type\":\"isomorphic\"}]\n```";
        let drafts = parse_drafts(raw).expect("围栏 JSON 应可解析");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].answer, "2");
    }

    #[test]
    fn parse_drafts_rejects_non_json() {
        assert!(parse_drafts("完全没有 JSON").is_err());
    }

    #[test]
    fn parse_drafts_applies_defaults() {
        let raw = "[{\"content\":\"缺少难度与类型字段的草稿题目\",\"answer\":\"x=1\"}]";
        let drafts = parse_drafts(raw).unwrap();
        assert_eq!(drafts[0].difficulty, 3);
        assert_eq!(drafts[0].variant_type, "isomorphic");
    }

    /// F-15 学习闭环集成：作答 → 判分 → attempt 落库 → EMA 更新 → 原题排复习。
    #[test]
    fn submit_attempt_closes_learning_loop() {
        let pool = DbPool::test_pool("variant-loop").unwrap();
        {
            let conn = pool.conn().unwrap();
            conn.execute(
                "INSERT INTO kg_node(subject_id, code, name, level) VALUES (1, 'M.T', '测试节点', 3)",
                [],
            )
            .unwrap();
            crate::db::dao::mistake::MistakeDao::new(&conn)
                .insert(1, "images/a.png")
                .unwrap();
        }
        let variant_id = {
            let conn = pool.conn().unwrap();
            VariantDao::new(&conn)
                .insert(
                    1,
                    Some(1),
                    &VariantDraft {
                        content: "求 1+1 的值并说明理由".into(),
                        content_latex: None,
                        answer: "2".into(),
                        solution: Some("加法".into()),
                        difficulty: 1,
                        variant_type: "isomorphic".into(),
                        validate_status: "passed".into(),
                        variant_id: None,
                    },
                    Some("test-model"),
                    None,
                )
                .unwrap()
        };

        let gateway = Arc::new(Gateway::new(pool.clone()));
        let gen = VariantGenerator::new(pool.clone(), gateway);
        let fb = gen.submit_attempt(variant_id, "答案：2", Some(1500), 1).unwrap();
        assert!(fb.is_correct);
        // mastery_delta = 0.65 - 0.5 = 0.15
        assert!((fb.mastery_delta.unwrap() - 0.15).abs() < 1e-9);

        let conn = pool.conn().unwrap();
        // attempt 落库
        let attempts: i64 = conn
            .query_row("SELECT COUNT(*) FROM attempt WHERE target_type='variant' AND target_id=?1",
                       [variant_id], |r| r.get(0))
            .unwrap();
        assert_eq!(attempts, 1);
        // 掌握度已建
        let m = MasteryDao::new(&conn).get(1, 1).unwrap().unwrap();
        assert!((m.score - 0.65).abs() < 1e-9);
        // 原错题已排入复习队列（默认 1/3/7/15/30 首段 +1 天）
        let plans = crate::db::dao::review::ReviewDao::new(&conn)
            .plans_for_target("mistake", 1)
            .unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].stage, 0);
    }

    /// 答错也应推进闭环（delta 为负）。
    #[test]
    fn submit_attempt_wrong_answer() {
        let pool = DbPool::test_pool("variant-wrong").unwrap();
        {
            let conn = pool.conn().unwrap();
            crate::db::dao::mistake::MistakeDao::new(&conn)
                .insert(1, "images/a.png")
                .unwrap();
        }
        let variant_id = {
            let conn = pool.conn().unwrap();
            VariantDao::new(&conn)
                .insert(
                    1,
                    None, // 无知识点 → mastery_delta 为 None
                    &VariantDraft {
                        content: "求 1+1 的值并说明理由".into(),
                        content_latex: None,
                        answer: "2".into(),
                        solution: None,
                        difficulty: 1,
                        variant_type: "isomorphic".into(),
                        validate_status: "passed".into(),
                        variant_id: None,
                    },
                    None,
                    None,
                )
                .unwrap()
        };
        let gateway = Arc::new(Gateway::new(pool.clone()));
        let gen = VariantGenerator::new(pool.clone(), gateway);
        let fb = gen.submit_attempt(variant_id, "3", None, 1).unwrap();
        assert!(!fb.is_correct);
        assert!(fb.mastery_delta.is_none(), "无知识点定位时不更新掌握度");
        assert!(fb.feedback.contains("参考答案"), "答错反馈应含参考答案");
    }
}
