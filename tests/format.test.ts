// format.ts 工具函数单元测试
import { describe, it, expect } from "vitest";
import {
  fmtDateTime,
  fmtDate,
  fmtCost,
  statusText,
  statusColor,
  difficultyText,
  variantTypeText,
  validateStatusText,
  originText,
  fmtBytes,
} from "../src/utils/format";

describe("fmtCost（分 → 元）", () => {
  it("零成本", () => {
    expect(fmtCost(0)).toBe("¥0.0000");
  });
  it("1 分 = ¥0.01", () => {
    expect(fmtCost(1)).toBe("¥0.0100");
  });
  it("123.456 分", () => {
    expect(fmtCost(123.456)).toBe("¥1.2346");
  });
});

describe("statusText / statusColor", () => {
  it("覆盖全部合法状态", () => {
    expect(statusText("pending_analysis")).toBe("待深度分析");
    expect(statusText("analyzing")).toBe("分析中");
    expect(statusText("analyzed")).toBe("已分析");
    expect(statusText("conflict")).toBe("冲突待合并");
    expect(statusText("archived")).toBe("已归档");
  });
  it("未知状态原样返回", () => {
    expect(statusText("unknown_x")).toBe("unknown_x");
  });
  it("颜色映射", () => {
    expect(statusColor("pending_analysis")).toBe("warning");
    expect(statusColor("conflict")).toBe("error");
    expect(statusColor("analyzed")).toBe("success");
    expect(statusColor("未知")).toBe("default");
  });
});

describe("difficultyText", () => {
  it("正常范围", () => {
    expect(difficultyText(3)).toBe("★★★");
  });
  it("钳制下限 1", () => {
    expect(difficultyText(0)).toBe("★");
    expect(difficultyText(-2)).toBe("★");
  });
  it("钳制上限 5", () => {
    expect(difficultyText(9)).toBe("★★★★★");
  });
});

describe("variantTypeText / validateStatusText / originText", () => {
  it("变式类型", () => {
    expect(variantTypeText("isomorphic")).toBe("同构巩固");
    expect(variantTypeText("recontext")).toBe("异景迁移");
    expect(variantTypeText("advanced")).toBe("递进提升");
    expect(variantTypeText("x")).toBe("x");
  });
  it("校验状态", () => {
    expect(validateStatusText("passed")).toBe("校验通过");
    expect(validateStatusText("manual_review")).toBe("人工复核");
  });
  it("归因来源", () => {
    expect(originText("local")).toBe("本地初判");
    expect(originText("cloud")).toBe("云端深判");
    expect(originText("user")).toBe("人工确认");
  });
});

describe("fmtBytes", () => {
  it("B / KB / MB 三档", () => {
    expect(fmtBytes(512)).toBe("512 B");
    expect(fmtBytes(2048)).toBe("2.0 KB");
    expect(fmtBytes(3 * 1024 * 1024)).toBe("3.0 MB");
  });
});

describe("日期格式化", () => {
  it("空值兜底为 -", () => {
    expect(fmtDateTime(null)).toBe("-");
    expect(fmtDateTime(undefined)).toBe("-");
    expect(fmtDate("")).toBe("-");
  });
  it("ISO 字符串可解析（正午时刻对任意时区稳定）", () => {
    expect(fmtDate("2025-01-02T12:00:00Z")).toBe("2025-01-02");
    expect(fmtDateTime("2025-01-02T12:00:00Z")).toMatch(/^\d{2}-\d{2} \d{2}:\d{2}$/);
  });
  it("SQLite datetime('now') 格式（无 Z）按 UTC 解析为本地", () => {
    // 后端统一 UTC 存储（无 Z），前端按 UTC→本地 转换；正午时刻跨时区不改日期
    expect(fmtDate("2025-01-02 12:00:00")).toBe("2025-01-02");
  });
});
