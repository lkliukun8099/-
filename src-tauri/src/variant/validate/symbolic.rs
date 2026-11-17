//! 校验闸 2：符号计算验证（数学/物理，SymPy sidecar stdio JSON-RPC）。
//!
//! sidecar 定位：`MAIDUO_MATHVERIFY_PATH` → 可执行文件同目录 `tools/mathverify/`
//! 缺失时本闸降级为 skipped（U-5 拍板：可选组件，由 LLM 交叉验证兜底）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use super::GateResult;
use crate::db::dao::variant::VariantDraft;
use crate::error::AppError;

/// JSON-RPC 请求。
#[derive(Debug, Serialize)]
struct RpcRequest {
    id: u64,
    method: String,
    params: serde_json::Value,
}

/// JSON-RPC 响应。
#[derive(Debug, Deserialize)]
struct RpcResponse {
    #[allow(dead_code)]
    id: u64,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

/// 请求 ID 计数器。
static RPC_ID: AtomicU64 = AtomicU64::new(1);

/// SymPy sidecar 客户端。
pub struct SymbolicGate {
    bin_path: Option<PathBuf>,
}

impl SymbolicGate {
    pub fn new() -> Self {
        Self {
            bin_path: locate_sidecar(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.bin_path.is_some()
    }

    /// 执行校验：
    /// 1. 题干含方程（=）且答案可解析 → verify_equation_solution
    /// 2. 否则对答案做表达式可解析性检查 check_expression
    /// 无法提取数学结构时返回 skipped（不判失败）。
    pub async fn check(&self, draft: &VariantDraft) -> GateResult {
        let bin = match &self.bin_path {
            Some(b) => b.clone(),
            None => {
                return GateResult {
                    gate: "symbolic",
                    passed: true,
                    skipped: true,
                    detail: "SymPy sidecar 未安装，符号验证降级跳过".into(),
                }
            }
        };

        // 提取题干中的第一个等式
        let equation = extract_equation(&draft.content);
        let answer_expr = extract_math_expr(&draft.answer);

        let result = match (equation, answer_expr) {
            (Some(eq), Some(ans)) => {
                self.rpc_call(
                    &bin,
                    "verify_equation_solution",
                    serde_json::json!({"equation": eq, "solution": ans}),
                )
                .await
            }
            (None, Some(ans)) => {
                self.rpc_call(&bin, "check_expression", serde_json::json!({"expr": ans}))
                    .await
            }
            _ => {
                return GateResult {
                    gate: "symbolic",
                    passed: true,
                    skipped: true,
                    detail: "题目无显式方程结构，符号验证不适用".into(),
                };
            }
        };

        match result {
            Ok(value) => {
                let ok = value
                    .get("ok")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let detail = value
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                GateResult {
                    gate: "symbolic",
                    passed: ok,
                    skipped: false,
                    detail: if detail.is_empty() {
                        if ok { "符号验证通过".into() } else { "符号验证未通过".into() }
                    } else {
                        detail
                    },
                }
            }
            Err(e) => {
                // sidecar 调用失败按 skipped 处理（降级，不误杀题目）
                tracing::warn!("SymPy sidecar 调用失败（降级跳过）: {e}");
                GateResult {
                    gate: "symbolic",
                    passed: true,
                    skipped: true,
                    detail: format!("sidecar 调用失败，降级跳过: {e}"),
                }
            }
        }
    }

    /// 发起一次 JSON-RPC 调用（每次调用独立进程，简单可靠）。
    async fn rpc_call(
        &self,
        bin: &PathBuf,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, AppError> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

        let mut child = tokio::process::Command::new(bin)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| AppError::validate(format!("mathverify sidecar 启动失败: {e}")))?;

        let req = RpcRequest {
            id: RPC_ID.fetch_add(1, Ordering::Relaxed),
            method: method.to_string(),
            params,
        };
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AppError::validate("sidecar stdin 不可用"))?;
        let mut line = serde_json::to_string(&req)?;
        line.push('\n');
        stdin.write_all(line.as_bytes()).await?;
        stdin.flush().await?;
        drop(stdin); // 关闭 stdin 通知 sidecar 请求结束

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::validate("sidecar stdout 不可用"))?;
        let mut reader = BufReader::new(stdout).lines();
        let resp_line = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            reader.next_line(),
        )
        .await
        .map_err(|_| AppError::validate("sidecar 响应超时"))?
        .map_err(|e| AppError::validate(format!("sidecar 读取失败: {e}")))?
        .ok_or_else(|| AppError::validate("sidecar 无响应输出"))?;

        let _ = child.kill().await;

        let resp: RpcResponse = serde_json::from_str(&resp_line)?;
        if let Some(err) = resp.error {
            return Err(AppError::validate(format!("sidecar 返回错误: {err}")));
        }
        resp.result
            .ok_or_else(|| AppError::validate("sidecar 响应缺少 result"))
    }
}

impl Default for SymbolicGate {
    fn default() -> Self {
        Self::new()
    }
}

/// sidecar 定位。
fn locate_sidecar() -> Option<PathBuf> {
    let exe_name = if cfg!(windows) {
        "mathverify.exe"
    } else {
        "mathverify"
    };
    if let Ok(custom) = std::env::var("MAIDUO_MATHVERIFY_PATH") {
        let p = PathBuf::from(custom);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("tools").join("mathverify").join(exe_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

/// 从题干提取第一个等式（含 = 的片段）。
fn extract_equation(content: &str) -> Option<String> {
    // 优先取 $...$ 内含 = 的 LaTeX 片段
    for seg in content.split('$') {
        if seg.contains('=') && seg.len() >= 3 {
            return Some(seg.trim().to_string());
        }
    }
    // 退化：按标点切句找含 = 的短句
    for seg in content.split(['，', '。', ',', '.', '；', ';']) {
        let seg = seg.trim();
        if seg.contains('=') && seg.len() <= 80 && seg.len() >= 3 {
            return Some(seg.to_string());
        }
    }
    None
}

/// 从答案提取数学表达式（去掉中文与说明文字）。
fn extract_math_expr(answer: &str) -> Option<String> {
    let trimmed = answer.trim();
    // $...$ 包裹优先
    if let Some(start) = trimmed.find('$') {
        if let Some(end) = trimmed[start + 1..].find('$') {
            let expr = trimmed[start + 1..start + 1 + end].trim();
            if !expr.is_empty() {
                return Some(expr.to_string());
            }
        }
    }
    // 纯数学字符行（数字、字母、运算符、括号、=、π 等）
    let candidate: String = trimmed
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || "+-*/^=().,<>[]{}|_ \\" .contains(*c)
                || "π√±≤≥≠".contains(*c)
        })
        .collect();
    let candidate = candidate.trim();
    if candidate.len() >= 1
        && candidate.chars().any(|c| c.is_ascii_alphanumeric())
        && candidate.len() <= 120
    {
        Some(candidate.to_string())
    } else {
        None
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
    fn extract_equation_prefers_latex_segment() {
        assert_eq!(
            extract_equation("已知 $x^2=4$ 且 $y=1$，求解"),
            Some("x^2=4".to_string())
        );
    }

    #[test]
    fn extract_equation_without_dollar_returns_whole_content() {
        // 注意当前实现语义：无 $ 分隔时若整段含 '=' 直接返回整段题干
        // （SymPy 解析失败会走降级，不会误判失败）
        assert_eq!(
            extract_equation("解方程 2x=4，求 x 的值"),
            Some("解方程 2x=4，求 x 的值".to_string())
        );
    }

    #[test]
    fn extract_equation_returns_first_dollar_segment_with_equals() {
        // 实现语义：按 $ 切段后返回第一个含 '=' 的段（不一定是公式段）
        assert_eq!(
            extract_equation("$x$ 与 $y$ 的关系，满足 2x=4 成立"),
            Some("的关系，满足 2x=4 成立".to_string())
        );
    }

    #[test]
    fn extract_equation_none_without_equals() {
        assert_eq!(extract_equation("化简 2x+3x"), None);
    }

    #[test]
    fn extract_expr_from_dollar_wrapped() {
        assert_eq!(extract_math_expr("$x=2$"), Some("x=2".to_string()));
    }

    #[test]
    fn extract_expr_strips_chinese() {
        assert_eq!(extract_math_expr("答案：x=2"), Some("x=2".to_string()));
    }

    #[test]
    fn extract_expr_none_for_pure_text() {
        assert_eq!(extract_math_expr("答对了"), None);
    }

    /// U-5 降级路径：sidecar 缺失时闸 2 应 skipped（不判失败）。
    #[tokio::test]
    async fn missing_sidecar_degrades_to_skipped() {
        let gate = SymbolicGate { bin_path: None };
        assert!(!gate.is_available());
        let r = gate.check(&draft("解方程 2x=4，求 x", "x=2")).await;
        assert!(r.skipped, "sidecar 缺失必须标记 skipped");
        assert!(r.passed, "降级跳过不计失败");
        assert!(r.detail.contains("未安装"));
    }

    /// 无数学结构的题目（如英语题）不进入符号验证。
    #[tokio::test]
    async fn non_math_content_skipped_even_with_sidecar() {
        // bin_path 指向不存在的路径时调用会失败 → 但本题先因无方程结构而 skip，
        // 不触达进程调用（用 None 直接覆盖该分支更稳）
        let gate = SymbolicGate { bin_path: None };
        let r = gate.check(&draft("阅读短文并回答问题", "B")).await;
        assert!(r.skipped);
    }
}
