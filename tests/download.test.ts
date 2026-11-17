// download.ts 二进制转换测试（downloadDataUrl 依赖 DOM，node 环境下不测）
import { describe, it, expect } from "vitest";
import { dataUrlToUint8Array } from "../src/utils/download";

describe("dataUrlToUint8Array", () => {
  it("正确解码 base64 内容", () => {
    // "hello" -> aGVsbG8=
    const bytes = dataUrlToUint8Array("data:text/plain;base64,aGVsbG8=");
    expect(Array.from(bytes)).toEqual([104, 101, 108, 108, 111]);
  });
  it("空 dataURL 返回空数组", () => {
    expect(dataUrlToUint8Array("data:,").length).toBe(0);
  });
  it("二进制内容（PNG 魔数）", () => {
    // 0x89 0x50 0x4E 0x47 -> iVBORw==
    const bytes = dataUrlToUint8Array("data:image/png;base64,iVBORw==");
    expect(Array.from(bytes)).toEqual([0x89, 0x50, 0x4e, 0x47]);
  });
});
