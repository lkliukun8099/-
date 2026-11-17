// 格式化工具（时间统一 UTC 存储 → 前端本地时区展示，架构 §9-4）
import dayjs from "dayjs";
import utc from "dayjs/plugin/utc";
import type { MistakeStatus } from "../types";

dayjs.extend(utc);

/** 将后端时间（SQLite datetime，UTC，无 Z 后缀）解析为本地时区 */
function parseUtc(iso: string): dayjs.Dayjs {
  // 无显式时区标记的按 UTC 处理（与后端统一口径）
  return dayjs.utc(iso).local();
}

/** UTC → 本地 "MM-DD HH:mm" */
export function fmtDateTime(iso: string | null | undefined): string {
  if (!iso) return "-";
  return parseUtc(iso).format("MM-DD HH:mm");
}

/** UTC → 本地 "YYYY-MM-DD" */
export function fmtDate(iso: string | null | undefined): string {
  if (!iso) return "-";
  return parseUtc(iso).format("YYYY-MM-DD");
}

/** 分 → 元 */
export function fmtCost(cnyFen: number): string {
  return `¥${(cnyFen / 100).toFixed(4)}`;
}

/** 错题状态文案 */
export function statusText(status: MistakeStatus | string): string {
  const map: Record<string, string> = {
    pending_analysis: "待深度分析",
    analyzing: "分析中",
    analyzed: "已分析",
    conflict: "冲突待合并",
    archived: "已归档",
  };
  return map[status] ?? status;
}

/** 错题状态颜色 */
export function statusColor(
  status: MistakeStatus | string
): "default" | "warning" | "info" | "success" | "error" {
  const map: Record<string, "default" | "warning" | "info" | "success" | "error"> = {
    pending_analysis: "warning",
    analyzing: "info",
    analyzed: "success",
    conflict: "error",
    archived: "default",
  };
  return map[status] ?? "default";
}

/** 难度文案 */
export function difficultyText(d: number): string {
  return "★".repeat(Math.max(1, Math.min(5, d)));
}

/** 变式类型文案 */
export function variantTypeText(t: string): string {
  const map: Record<string, string> = {
    isomorphic: "同构巩固",
    recontext: "异景迁移",
    advanced: "递进提升",
  };
  return map[t] ?? t;
}

/** 校验状态文案 */
export function validateStatusText(s: string): string {
  const map: Record<string, string> = {
    pending: "校验中",
    passed: "校验通过",
    failed: "校验失败",
    manual_review: "人工复核",
  };
  return map[s] ?? s;
}

/** 归因来源文案 */
export function originText(origin: string): string {
  const map: Record<string, string> = {
    local: "本地初判",
    cloud: "云端深判",
    user: "人工确认",
  };
  return map[origin] ?? origin;
}

/** 文件大小 */
export function fmtBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
