//! 出题模板库：同构（巩固）/ 异景（迁移）/ 递进（提升）三类提示词模板 +
//! 按知识点前缀匹配的少样例（few-shot）库。

use crate::db::dao::mistake::Mistake;

/// 变式生成系统提示词。
pub const VARIANT_SYSTEM: &str = r#"你是一位严谨的中学数学命题老师。根据学生的错题生成变式题，要求：
1. 题目考查与原题相同的知识点，但数值、情境或设问角度有变化；
2. 每道题必须给出**正确答案**与**简要的解题过程**，答案必须可严格推导，不得编造；
3. 难度符合指定要求（1-5）；
4. 输出严格的 JSON 数组，每个元素格式：
   {"content":"题干","content_latex":"LaTeX 题干(无公式可为null)","answer":"答案","solution":"解析","difficulty":3,"variant_type":"isomorphic|recontext|advanced"}
只输出 JSON 数组，不要输出任何其他文字。"#;

/// 少样例（按知识点编码前缀匹配）。
struct FewShot {
    kg_prefix: &'static str,
    example_stem: &'static str,
    example_answer: &'static str,
}

const FEW_SHOTS: &[FewShot] = &[
    FewShot {
        kg_prefix: "MATH.SX.FUNC.FUNC.QUAD",
        example_stem: "已知 f(x)=x²-2x+3 在 [2,+∞) 上单调递增，验证其对称轴位置并说明理由。",
        example_answer: "对称轴 x=1 ≤ 2，故区间 [2,+∞) 在对称轴右侧，函数单调递增。",
    },
    FewShot {
        kg_prefix: "MATH.SX.NUM.NUM.EQ",
        example_stem: "解方程：x²-5x+6=0，并验证两根之和与两根之积。",
        example_answer: "x1=2, x2=3；x1+x2=5=-(-5)/1，x1·x2=6=6/1，符合韦达定理。",
    },
    FewShot {
        kg_prefix: "MATH.SX.GEO.GEO.CIRCLE",
        example_stem: "半径为 5 的圆中，圆心角 60° 所对的弧长是多少？",
        example_answer: "l = 60π×5/180 = 5π/3。",
    },
    FewShot {
        kg_prefix: "MATH.SX.STAT",
        example_stem: "从 1~6 中任取两个不同的数，求两数之和为 7 的概率。",
        example_answer: "基本事件 C(6,2)=15，和为 7 的组合 (1,6)(2,5)(3,4) 共 3 个，P=3/15=1/5。",
    },
];

/// 组装生成提示词。
///
/// - `count`：生成数量（默认同构/递进各一）
/// - `kg_path`：知识点路径（如 函数/二次函数/单调性）
/// - `kg_code`：三级节点编码（用于匹配少样例）
/// - `extra_instruction`：校验失败重生成时携带的失败原因
pub fn build_generate_prompt(
    mistake: &Mistake,
    count: u32,
    kg_path: &[String],
    kg_code: Option<&str>,
    extra_instruction: Option<&str>,
) -> String {
    let stem = mistake.ocr_text.as_deref().unwrap_or("（未识别）");
    let correct = mistake.correct_answer.as_deref().unwrap_or("（未提供）");
    let path_str = if kg_path.is_empty() {
        "（未定位）".to_string()
    } else {
        kg_path.join(" → ")
    };

    // 少样例匹配
    let few_shot = kg_code
        .and_then(|code| {
            FEW_SHOTS
                .iter()
                .find(|fs| code.starts_with(fs.kg_prefix) || fs.kg_prefix.starts_with(code))
        })
        .map(|fs| {
            format!(
                "\n【同知识点范例】\n题目：{}\n答案：{}\n",
                fs.example_stem, fs.example_answer
            )
        })
        .unwrap_or_default();

    let retry = extra_instruction
        .map(|r| format!("\n【上次生成未通过校验，原因】\n{r}\n请针对性修正后重新出题。\n"))
        .unwrap_or_default();

    format!(
        "【原错题】\n{stem}\n\n【正确答案】\n{correct}\n\n【考查知识点】\n{path_str}\n{few_shot}{retry}\n请生成 {count} 道变式题：\
         同构巩固（variant_type=isomorphic，难度与原题相当）与递进提升（variant_type=advanced，难度+1）合理搭配，\
         输出 JSON 数组。"
    )
}

/// LLM 输出的变式草稿 JSON 结构。
#[derive(Debug, serde::Deserialize)]
pub struct LlmVariantDraft {
    pub content: String,
    pub content_latex: Option<String>,
    pub answer: String,
    pub solution: Option<String>,
    #[serde(default = "default_difficulty")]
    pub difficulty: i64,
    #[serde(default = "default_variant_type")]
    pub variant_type: String,
}

fn default_difficulty() -> i64 {
    3
}
fn default_variant_type() -> String {
    "isomorphic".to_string()
}

/// 从 LLM 输出中提取 JSON 数组。
pub fn extract_json_array(raw: &str) -> Result<&str, crate::error::AppError> {
    let start = raw.find('[');
    let end = raw.rfind(']');
    match (start, end) {
        (Some(s), Some(e)) if e > s => Ok(&raw[s..=e]),
        _ => Err(crate::error::AppError::validate(
            "LLM 输出中未找到 JSON 数组",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::dao::mistake::Mistake;

    fn sample_mistake() -> Mistake {
        Mistake {
            id: 1,
            profile_id: 1,
            subject_id: Some(1),
            subject_name: None,
            source_image: "images/a.png".into(),
            processed_image: None,
            ocr_text: Some("已知 f(x)=x²-2ax+3 在 [1,2] 上单调递减".into()),
            ocr_latex: None,
            ocr_engine: None,
            ocr_confidence: None,
            student_answer: Some("a≥1".into()),
            correct_answer: Some("a≥2".into()),
            chapter: None,
            question_type: None,
            difficulty: 3,
            source: None,
            status: "analyzed".into(),
            version: 1,
            created_at: "2025-01-01 00:00:00".into(),
            updated_at: "2025-01-01 00:00:00".into(),
        }
    }

    #[test]
    fn prompt_contains_stem_and_answers() {
        let p = build_generate_prompt(&sample_mistake(), 2, &["函数".into(), "二次函数".into()], None, None);
        assert!(p.contains("单调递减"), "提示词应含题干");
        assert!(p.contains("a≥2"), "提示词应含正确答案");
        assert!(p.contains("函数 → 二次函数"), "提示词应含知识路径");
        assert!(p.contains("2 道"), "提示词应含数量");
    }

    #[test]
    fn prompt_matches_few_shot_by_kg_prefix() {
        let with = build_generate_prompt(
            &sample_mistake(),
            2,
            &[],
            Some("MATH.SX.FUNC.FUNC.QUAD.T03"),
            None,
        );
        assert!(with.contains("同知识点范例"), "QUAD 前缀应命中少样例");
        let without = build_generate_prompt(&sample_mistake(), 2, &[], None, None);
        assert!(!without.contains("同知识点范例"));
    }

    #[test]
    fn prompt_carries_retry_reason() {
        let p = build_generate_prompt(&sample_mistake(), 2, &[], None, Some("符号验证未通过"));
        assert!(p.contains("上次生成未通过校验"));
        assert!(p.contains("符号验证未通过"));
    }

    #[test]
    fn extract_json_array_tolerates_noise() {
        assert_eq!(
            extract_json_array("前言[{\"a\":1}]后记").unwrap(),
            "[{\"a\":1}]"
        );
        assert!(extract_json_array("没有数组").is_err());
        assert!(extract_json_array("[").is_err(), "缺少闭括号应拒绝");
        assert_eq!(extract_json_array("[]").unwrap(), "[]");
    }
}
