// client.ts 统一错误处理测试（架构 §9-1：AppError unwrap + 全局弹错 + silent 模式）
import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { call } from "../src/api/client";
import { useAppStore } from "../src/stores/appStore";

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockedInvoke.mockClear();
  useAppStore.setState({ notice: null });
});

describe("call 统一封装", () => {
  it("成功透传返回值", async () => {
    mockedInvoke.mockResolvedValue({ id: 1 });
    await expect(call<{ id: number }>("mistake_get", { id: 1 })).resolves.toEqual({ id: 1 });
  });

  it("AppError 失败：全局弹错含 code+message+detail，并继续抛出", async () => {
    mockedInvoke.mockRejectedValue({ code: 2004, message: "版本冲突", detail: "v1 != v2" });
    await expect(call("mistake_update", {})).rejects.toMatchObject({
      code: 2004,
      message: "版本冲突",
    });
    const notice = useAppStore.getState().notice;
    expect(notice?.severity).toBe("error");
    expect(notice?.message).toBe("[2004] 版本冲突（v1 != v2）");
  });

  it("非结构化错误按 String 兜底弹错", async () => {
    mockedInvoke.mockRejectedValue("raw string error");
    await call("x").catch(() => {});
    expect(useAppStore.getState().notice?.message).toBe("raw string error");
  });

  it("silent=true 不弹错但仍抛出", async () => {
    mockedInvoke.mockRejectedValue({ code: 1002, message: "不存在" });
    await expect(call("settings_get", {}, true)).rejects.toBeTruthy();
    expect(useAppStore.getState().notice).toBeNull();
  });
});
