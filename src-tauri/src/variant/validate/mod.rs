//! 三段校验管道（F-09）：规则 → 符号计算 → LLM 自校验。
//!
//! 判定：任一未跳过的闸失败即 overall failed；全部通过/跳过 → passed。
//! 重试由生成编排层负责（≤2 次），最终仍失败标记 manual_review。

pub mod llm_check;
pub mod rules;
pub mod symbolic;

use std::sync::Arc;

use serde::Serialize;

use crate::ai::Gateway;
use crate::db::dao::variant::VariantDraft;

/// 单闸结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GateResult {
    pub gate: &'static str,
    pub passed: bool,
    /// 降级跳过（如 sidecar 缺失），不计失败
    pub skipped: bool,
    pub detail: String,
}

/// 三闸汇总报告（存 variant_question.validate_detail）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateReport {
    pub rules: GateResult,
    pub symbolic: GateResult,
    pub llm: GateResult,
    pub final_status: String,
}

/// 校验管道。
pub struct ValidatePipeline {
    gateway: Arc<Gateway>,
    symbolic: symbolic::SymbolicGate,
}

impl ValidatePipeline {
    pub fn new(gateway: Arc<Gateway>) -> Self {
        Self {
            gateway,
            symbolic: symbolic::SymbolicGate::new(),
        }
    }

    /// 执行三段校验。
    pub async fn validate(&self, draft: &VariantDraft) -> ValidateReport {
        // 闸 1：规则（失败则短路，不再浪费下游调用）
        let rules_result = rules::check(draft);
        if !rules_result.passed {
            return ValidateReport {
                rules: rules_result,
                symbolic: GateResult {
                    gate: "symbolic",
                    passed: true,
                    skipped: true,
                    detail: "闸1 未通过，跳过".into(),
                },
                llm: GateResult {
                    gate: "llm_check",
                    passed: true,
                    skipped: true,
                    detail: "闸1 未通过，跳过".into(),
                },
                final_status: "failed".into(),
            };
        }

        // 闸 2：符号计算（数学/物理）
        let symbolic_result = self.symbolic.check(draft).await;

        // 闸 3：LLM 交叉校验
        let llm_result = llm_check::check(&self.gateway, draft).await;

        let failed = (!rules_result.passed)
            || (!symbolic_result.skipped && !symbolic_result.passed)
            || (!llm_result.skipped && !llm_result.passed);

        ValidateReport {
            rules: rules_result,
            symbolic: symbolic_result,
            llm: llm_result,
            final_status: if failed { "failed" } else { "passed" }.into(),
        }
    }

    /// 报告的可读摘要（重生成时反馈给 LLM）。
    pub fn failure_summary(report: &ValidateReport) -> String {
        let mut parts = Vec::new();
        if !report.rules.passed {
            parts.push(format!("规则校验: {}", report.rules.detail));
        }
        if !report.symbolic.skipped && !report.symbolic.passed {
            parts.push(format!("符号验证: {}", report.symbolic.detail));
        }
        if !report.llm.skipped && !report.llm.passed {
            parts.push(format!("交叉校验: {}", report.llm.detail));
        }
        if parts.is_empty() {
            "未知原因".to_string()
        } else {
            parts.join("；")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbPool;

    fn draft(content: &str, answer: &str) -> VariantDraft {
        VariantDraft {
            content: content.to_string(),
            content_latex: None,
            answer: answer.to_string(),
            solution: None,
            difficulty: 3,
            variant_type: "isomorphic".into(),
            validate_status: "pending".into(),
            variant_id: None,
        }
    }

    fn pipeline(tag: &str) -> ValidatePipeline {
        let pool = DbPool::test_pool(tag).unwrap();
        ValidatePipeline::new(Arc::new(Gateway::new(pool)))
    }

    /// 闸 1 失败必须短路：闸 2/3 标记 skipped 且不触达 sidecar/网络。
    #[tokio::test]
    async fn rules_failure_short_circuits() {
        let p = pipeline("vp-short");
        let report = p.validate(&draft("短", "")).await;
        assert_eq!(report.final_status, "failed");
        assert!(!report.rules.passed);
        assert!(report.symbolic.skipped);
        assert!(report.llm.skipped);
        assert!(ValidatePipeline::failure_summary(&report).contains("规则校验"));
    }

    /// 全降级路径：无 sidecar（闸 2 skip）+ 无可用模型（闸 3 skip）→ 不判失败。
    /// 前提：测试环境未配置任何 provider 密钥（闸 3 走 NO_KEY 降级）。
    /// 题干刻意不含 '='，使有 sidecar 的环境也只做表达式检查（"5" 可解析 → pass）。
    #[tokio::test]
    async fn all_gates_degraded_still_passes() {
        let p = pipeline("vp-degrade");
        let report = p
            .validate(&draft("求 2+3 的值并说明计算过程。", "5"))
            .await;
        assert!(report.rules.passed, "rules: {}", report.rules.detail);
        assert!(report.symbolic.skipped || report.symbolic.passed);
        // 无密钥/熔断 → llm 闸降级 skipped（llm_check 对错误一律降级）
        assert!(report.llm.skipped || report.llm.passed);
        assert_eq!(report.final_status, "passed", "降级不误杀题目（U-5）");
    }

    #[test]
    fn failure_summary_collects_failed_gates() {
        let report = ValidateReport {
            rules: GateResult { gate: "rules", passed: false, skipped: false, detail: "题干过短".into() },
            symbolic: GateResult { gate: "symbolic", passed: false, skipped: false, detail: "代入不成立".into() },
            llm: GateResult { gate: "llm_check", passed: true, skipped: true, detail: "降级".into() },
            final_status: "failed".into(),
        };
        let s = ValidatePipeline::failure_summary(&report);
        assert!(s.contains("规则校验"));
        assert!(s.contains("符号验证"));
        assert!(!s.contains("交叉校验"), "skipped 的闸不进摘要");
    }

    #[test]
    fn failure_summary_empty_when_nothing_failed() {
        let report = ValidateReport {
            rules: GateResult { gate: "rules", passed: true, skipped: false, detail: "ok".into() },
            symbolic: GateResult { gate: "symbolic", passed: true, skipped: true, detail: "skip".into() },
            llm: GateResult { gate: "llm_check", passed: true, skipped: true, detail: "skip".into() },
            final_status: "passed".into(),
        };
        assert_eq!(ValidatePipeline::failure_summary(&report), "未知原因");
    }
}
