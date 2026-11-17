//! 归因与自校验提示词模板（内容最小化：仅题目与作答，不含任何 PII）。

use crate::db::dao::mistake::Mistake;

/// 归因系统提示词。
pub const ATTRIBUTION_SYSTEM: &str = r#"你是一位资深中学教研老师，擅长分析学生错题的深层原因。
请根据题干、学生作答与正确答案，输出错因归因，要求：
1. cause_label：具体、可操作的错因标签（不超过 20 字），禁止"概念不清""粗心"等泛泛标签；
2. cause_detail：可解释的归因说明（50-150 字），指出具体思维偏差；
3. kg_code：从给定知识图谱候选编码中选择最匹配的一个；无匹配时输出 null；
4. confidence：0~1 的置信度。
只输出 JSON，格式：{"cause_label":"...","cause_detail":"...","kg_code":"...|null","confidence":0.8}"#;

/// 组装归因用户提示词。
///
/// `kg_candidates`：图谱候选（编码+名称），由调用方按学科/章节预筛。
pub fn build_attribution_prompt(mistake: &Mistake, kg_candidates: &[(String, String)]) -> String {
    let stem = mistake.ocr_text.as_deref().unwrap_or("（未识别）");
    let latex = mistake.ocr_latex.as_deref().unwrap_or("");
    let student = mistake.student_answer.as_deref().unwrap_or("（未填写）");
    let correct = mistake.correct_answer.as_deref().unwrap_or("（未提供）");

    let candidates = if kg_candidates.is_empty() {
        "（无候选）".to_string()
    } else {
        kg_candidates
            .iter()
            .map(|(code, name)| format!("- {code}: {name}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "【题干】\n{stem}\n{latex}\n\n【学生作答】\n{student}\n\n【正确答案】\n{correct}\n\n【知识图谱候选】\n{candidates}\n\n请输出归因 JSON。"
    )
}

/// 归因 LLM 输出的解析结构。
#[derive(Debug, serde::Deserialize)]
pub struct LlmAttribution {
    pub cause_label: String,
    pub cause_detail: Option<String>,
    pub kg_code: Option<String>,
    #[serde(default)]
    pub confidence: f64,
}

/// 从 LLM 输出中提取 JSON（容忍 ```json 围栏与首尾杂讯）。
pub fn extract_json(raw: &str) -> Result<&str, crate::error::AppError> {
    let start = raw.find('{');
    let end = raw.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => Ok(&raw[s..=e]),
        _ => Err(crate::error::AppError::attribution(
            "LLM 输出中未找到 JSON 内容",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_from_fenced_output() {
        let raw = "分析如下：\n```json\n{\"cause_label\":\"x\",\"confidence\":0.8}\n```\n完毕";
        let out = extract_json(raw).unwrap();
        assert!(out.starts_with('{') && out.ends_with('}'));
        let v: serde_json::Value = serde_json::from_str(out).unwrap();
        assert_eq!(v["cause_label"], "x");
    }

    #[test]
    fn extract_json_rejects_garbage() {
        assert!(extract_json("没有 JSON 内容").is_err());
        assert!(extract_json("{").is_err());
    }

    #[test]
    fn attribution_prompt_minimizes_content() {
        // 隐私（§6-3）：提示词只含题目与作答，不含任何 PII 字段
        let mistake = Mistake {
            id: 1,
            profile_id: 1,
            subject_id: None,
            subject_name: None,
            source_image: "images/student_zhangsan.png".into(),
            processed_image: None,
            ocr_text: Some("题干内容".into()),
            ocr_latex: None,
            ocr_engine: None,
            ocr_confidence: None,
            student_answer: Some("学生作答".into()),
            correct_answer: Some("正确答案".into()),
            chapter: None,
            question_type: None,
            difficulty: 3,
            source: None,
            status: "analyzed".into(),
            version: 1,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let prompt = build_attribution_prompt(&mistake, &[("MATH.T".into(), "节点".into())]);
        assert!(prompt.contains("题干内容"));
        assert!(prompt.contains("MATH.T"));
        assert!(!prompt.contains("zhangsan"), "图片路径等潜在 PII 不得进入提示词");
    }
}
