//! 校验闸 1：规则校验（题型完整性 / 数值合理性 / 敏感内容过滤）。

use once_cell::sync::Lazy;
use regex::Regex;

use super::GateResult;
use crate::db::dao::variant::VariantDraft;

/// 敏感内容正则（内容合规，PRD §6-5）
static RE_SENSITIVE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(暴力|色情|赌博|毒品|自杀|恐怖|邪教|枪支|爆炸物制作)").expect("敏感词正则编译失败")
});
/// AI 幻觉痕迹（模型自指、免责声明混入题干）
static RE_HALLUCINATION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(作为\s?AI|作为人工智能|我无法|我不能|抱歉[，,]?\s*我)").expect("幻觉正则编译失败")
});

/// 规则校验。
pub fn check(draft: &VariantDraft) -> GateResult {
    let mut issues: Vec<String> = Vec::new();

    // 1. 完整性
    let content = draft.content.trim();
    if content.chars().count() < 8 {
        issues.push("题干过短（少于 8 字），疑似不完整".into());
    }
    if content.chars().count() > 2000 {
        issues.push("题干超长（超过 2000 字）".into());
    }
    if draft.answer.trim().is_empty() {
        issues.push("答案为空".into());
    }
    if draft.answer.trim().chars().count() > 500 {
        issues.push("答案超长（超过 500 字）".into());
    }
    if !(1..=5).contains(&draft.difficulty) {
        issues.push(format!("难度 {} 超出 1-5 范围", draft.difficulty));
    }
    if !["isomorphic", "recontext", "advanced"].contains(&draft.variant_type.as_str()) {
        issues.push(format!("变式类型 {} 非法", draft.variant_type));
    }

    // 2. 数值合理性：题干中不应出现 NaN/Infinity 等异常数值
    if content.contains("NaN") || content.contains("Infinity") || content.contains("undefined") {
        issues.push("题干含非法数值（NaN/Infinity/undefined）".into());
    }

    // 3. LaTeX 结构基本健全性（括号与 $ 配对）
    if let Some(latex) = &draft.content_latex {
        let open = latex.matches('{').count();
        let close = latex.matches('}').count();
        if open != close {
            issues.push(format!("LaTeX 花括号不配对（{open} 对 {close}）"));
        }
        let dollars = latex.matches('$').count();
        if dollars % 2 != 0 {
            issues.push("LaTeX $ 定界符不配对".into());
        }
    }

    // 4. 敏感内容与幻觉痕迹
    let full_text = format!("{} {}", draft.content, draft.answer);
    if RE_SENSITIVE.is_match(&full_text) {
        issues.push("内容命中敏感词过滤".into());
    }
    if RE_HALLUCINATION.is_match(&full_text) {
        issues.push("内容含 AI 幻觉痕迹（模型自指表述）".into());
    }

    GateResult {
        gate: "rules",
        passed: issues.is_empty(),
        skipped: false,
        detail: if issues.is_empty() {
            "规则校验通过".to_string()
        } else {
            issues.join("；")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn valid_draft_passes() {
        let r = check(&draft("已知 x+1=3，求 x 的值。", "x=2"));
        assert!(r.passed, "合法草稿应通过: {}", r.detail);
        assert!(!r.skipped);
        assert_eq!(r.gate, "rules");
    }

    #[test]
    fn short_content_fails() {
        let r = check(&draft("1+1=?", "2"));
        assert!(!r.passed);
        assert!(r.detail.contains("题干过短"));
    }

    #[test]
    fn empty_answer_fails() {
        let r = check(&draft("这是一道完整的题目内容", "  "));
        assert!(!r.passed);
        assert!(r.detail.contains("答案为空"));
    }

    #[test]
    fn difficulty_out_of_range_fails() {
        let mut d = draft("这是一道完整的题目内容", "2");
        d.difficulty = 0;
        assert!(!check(&d).passed);
        d.difficulty = 6;
        assert!(!check(&d).passed);
    }

    #[test]
    fn illegal_variant_type_fails() {
        let mut d = draft("这是一道完整的题目内容", "2");
        d.variant_type = "random".into();
        let r = check(&d);
        assert!(!r.passed);
        assert!(r.detail.contains("非法"));
    }

    #[test]
    fn latex_brace_mismatch_fails() {
        let mut d = draft("这是一道完整的题目内容", "2");
        d.content_latex = Some("\\frac{1}{2".into());
        let r = check(&d);
        assert!(!r.passed);
        assert!(r.detail.contains("花括号不配对"));
    }

    #[test]
    fn latex_dollar_mismatch_fails() {
        let mut d = draft("这是一道完整的题目内容", "2");
        d.content_latex = Some("$x=1".into());
        assert!(!check(&d).passed);
    }

    #[test]
    fn sensitive_content_blocked() {
        // 内容合规（PRD §6-5）
        let r = check(&draft("编写一个关于赌博的数学概率应用题", "1/6"));
        assert!(!r.passed);
        assert!(r.detail.contains("敏感词"));
    }

    #[test]
    fn hallucination_trace_blocked() {
        let r = check(&draft("作为AI我无法给出这道题的标准答案内容", "无"));
        assert!(!r.passed);
        assert!(r.detail.contains("幻觉"));
    }

    #[test]
    fn invalid_numeric_literal_blocked() {
        let r = check(&draft("计算表达式 NaN 的值并化简", "0"));
        assert!(!r.passed);
        assert!(r.detail.contains("非法数值"));
    }
}
