// 前端 PII 脱敏辅助（展示层防御性打码；主脱敏在 Rust logging 层，架构 §9-7）

const RE_PHONE = /1[3-9]\d{9}/g;
const RE_EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g;
const RE_ID_CARD = /\d{17}[\dXx]/g;

/** 对展示文本做 PII 打码（长模式优先：身份证先于手机号，防子串截胡） */
export function maskPii(text: string): string {
  return text
    .replace(RE_ID_CARD, "[身份证号]")
    .replace(RE_PHONE, "[手机号]")
    .replace(RE_EMAIL, "[邮箱]");
}
