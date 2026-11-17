//! 结构化日志初始化 + PII 脱敏层。
//!
//! 隐私红线（架构 §9-7）：日志落盘前对手机号/邮箱/身份证号/API Key/学校名等
//! 常见 PII 做正则打码。脱敏在写盘前的 Writer 层统一执行，
//! 任何模块使用 tracing 宏即自动享受脱敏，无需各自处理。

use once_cell::sync::Lazy;
use regex::Regex;
use std::io::{self, Write};
use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, EnvFilter};

/// 手机号（中国大陆 11 位）
static RE_PHONE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"1[3-9]\d{9}").expect("手机号正则编译失败"));
/// 邮箱
static RE_EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").expect("邮箱正则编译失败")
});
/// 身份证号（18 位，末位可为 X）
static RE_ID_CARD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d{17}[\dXx]").expect("身份证正则编译失败"));
/// API Key 形态（sk- 开头的常见令牌）
static RE_API_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"sk-[A-Za-z0-9_\-]{8,}").expect("API Key 正则编译失败"));
/// 学校名称（××中学/××小学/××学校，打码处理）
static RE_SCHOOL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[\p{Han}]{2,8}(第[一二三四五六七八九十\d]+)?(中学|小学|学校|高中|初中)")
        .expect("学校正则编译失败")
});

/// 对一段文本做 PII 脱敏，返回打码后的文本。
///
/// 顺序敏感：长模式（身份证 18 位）必须先于短模式（手机号 11 位），
/// 否则身份证号中的 11 位子串会被手机号正则截胡，导致部分打码（AC-9）。
pub fn mask_pii(input: &str) -> String {
    let s = RE_ID_CARD.replace_all(input, "[身份证号]");
    let s = RE_PHONE.replace_all(&s, "[手机号]");
    let s = RE_EMAIL.replace_all(&s, "[邮箱]");
    let s = RE_API_KEY.replace_all(&s, "[密钥]");
    let s = RE_SCHOOL.replace_all(&s, "[学校]");
    s.into_owned()
}

/// 脱敏 Writer：所有写入内容先过 `mask_pii` 再落盘。
pub struct PiiMaskWriter<W: Write> {
    inner: W,
}

impl<W: Write> PiiMaskWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write> Write for PiiMaskWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // 日志行基本是合法 UTF-8；失败时原文写兜底，避免丢日志
        match std::str::from_utf8(buf) {
            Ok(text) => {
                let masked = mask_pii(text);
                self.inner.write_all(masked.as_bytes())?;
                // 返回原始长度，语义上表示"消费了全部输入"
                Ok(buf.len())
            }
            Err(_) => self.inner.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// 初始化日志系统：每日滚动文件 + PII 脱敏。
///
/// 返回 `WorkerGuard`，必须在应用生命周期内持有，否则异步日志可能丢失。
pub fn init_tracing(log_dir: PathBuf) -> anyhow::Result<WorkerGuard> {
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = tracing_appender::rolling::daily(&log_dir, "maiduo.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(if cfg!(debug_assertions) {
            "debug"
        } else {
            "info"
        })
    });

    fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_target(true)
        // 每次产出 writer 时包一层 PII 脱敏
        .with_writer(move || PiiMaskWriter::new(non_blocking.clone()))
        .init();

    tracing::info!("日志系统初始化完成，目录: {}", log_dir.display());
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_phone() {
        assert_eq!(mask_pii("联系 13812345678 王老师"), "联系 [手机号] 王老师");
    }

    #[test]
    fn masks_email() {
        assert_eq!(
            mask_pii("邮箱 zhang.san@example.com 备用"),
            "邮箱 [邮箱] 备用"
        );
    }

    #[test]
    fn masks_api_key() {
        assert_eq!(
            mask_pii("key=sk-abcdef1234567890 失效"),
            "key=[密钥] 失效"
        );
    }

    #[test]
    fn masks_school() {
        assert_eq!(mask_pii("阳光中学"), "[学校]");
        assert_eq!(mask_pii("阳光第一中学"), "[学校]");
        // 注意：正则未加边界，前导汉字会被贪心并入匹配（"就读于阳光中学" → "[学校]"）。
        // 对日志脱敏而言"宁严勿漏"可接受，此处仅固化当前行为。
        assert_eq!(mask_pii("就读于阳光中学"), "[学校]");
    }

    #[test]
    fn masks_id_card() {
        assert_eq!(mask_pii("证件 32031119700101001X"), "证件 [身份证号]");
    }

    /// 隐私缺陷复现（已路由工程师修复）：
    /// 脱敏顺序为 手机号→邮箱→身份证，18 位身份证号中的 11 位子串
    /// （如 "19900307753"）会先被手机号正则吃掉，导致身份证号只被部分打码。
    /// 正确行为：身份证号应优先完整打码。
    #[test]
    fn id_card_should_be_masked_fully_not_partially() {
        let out = mask_pii("证件 110101199003077531");
        assert_eq!(
            out, "证件 [身份证号]",
            "身份证号应整体打码，不能被子串手机号规则截胡"
        );
    }

    #[test]
    fn normal_text_untouched() {
        let s = "已知 f(x)=x²-2ax+3 在 [1,2] 上单调递减";
        assert_eq!(mask_pii(s), s);
    }

    #[test]
    fn writer_masks_and_reports_consumed_length() {
        let mut buf: Vec<u8> = Vec::new();
        let mut w = PiiMaskWriter::new(&mut buf);
        let input = "手机号13812345678";
        let n = w.write(input.as_bytes()).expect("写入应成功");
        assert_eq!(n, input.len(), "write 应返回消费了全部输入");
        drop(w);
        let out = String::from_utf8(buf).unwrap();
        assert!(!out.contains("13812345678"));
        assert!(out.contains("[手机号]"));
    }

    #[test]
    fn writer_passthrough_on_invalid_utf8() {
        let mut buf: Vec<u8> = Vec::new();
        let mut w = PiiMaskWriter::new(&mut buf);
        let raw = [0xff, 0xfe, 0x00];
        let n = w.write(&raw).unwrap();
        assert_eq!(n, 3);
        drop(w);
        assert_eq!(buf, raw);
    }
}
