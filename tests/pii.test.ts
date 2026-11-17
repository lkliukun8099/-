// pii.ts 前端展示层脱敏测试（主脱敏在 Rust logging 层）
import { describe, it, expect } from "vitest";
import { maskPii } from "../src/utils/pii";

describe("maskPii", () => {
  it("手机号打码", () => {
    expect(maskPii("联系 13812345678 王老师")).toBe("联系 [手机号] 王老师");
  });
  it("非手机号不误伤（12 开头号段）", () => {
    expect(maskPii("编号 12345678901")).toBe("编号 12345678901"); // 12 开头不匹配
  });
  it("注意：长数字串中的 11 位手机号子串也会被打码（无边界锚定，宁严勿漏）", () => {
    // 当前实现未加边界断言，"138123456789" 中前 11 位会被识别为手机号打码
    expect(maskPii("数字 138123456789")).toBe("数字 [手机号]9");
  });
  it("邮箱打码", () => {
    expect(maskPii("邮箱 zhang.san@example.com 备用")).toBe("邮箱 [邮箱] 备用");
  });
  it("身份证号打码（含 X 末位）", () => {
    expect(maskPii("证件 11010119900307753X")).toBe("证件 [身份证号]");
    expect(maskPii("证件 110101199003077531")).toBe("证件 [身份证号]");
  });
  it("多类 PII 混合", () => {
    const out = maskPii("张三 13900001111，邮箱 a@b.cn，证件 110101199003077531");
    expect(out).not.toContain("13900001111");
    expect(out).not.toContain("a@b.cn");
    expect(out).not.toContain("110101199003077531");
  });
  it("普通数学文本不受影响", () => {
    const text = "已知 f(x)=x²-2ax+3 在 [1,2] 上单调递减";
    expect(maskPii(text)).toBe(text);
  });
});
