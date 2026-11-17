// invoke 统一封装：错误 unwrap + 全局弹错 + Channel 流式桥接（架构 §9-1/§9-6）
import { invoke, Channel } from "@tauri-apps/api/core";
import type { AppError, StreamChunk } from "../types";
import { useAppStore } from "../stores/appStore";

/** 是否为结构化 AppError */
function isAppError(e: unknown): e is AppError {
  return (
    typeof e === "object" &&
    e !== null &&
    "code" in e &&
    "message" in e
  );
}

/**
 * 统一命令调用：失败时全局弹错（code+message）并继续抛出。
 * `silent=true` 时不弹错（调用方自行处理）。
 */
export async function call<T>(
  cmd: string,
  args?: Record<string, unknown>,
  silent = false
): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    if (!silent) {
      const message = isAppError(e)
        ? `[${e.code}] ${e.message}${e.detail ? `（${e.detail}）` : ""}`
        : String(e);
      useAppStore.getState().notify({ severity: "error", message });
    }
    throw e;
  }
}

/**
 * 流式命令调用：Tauri 2 Channel 桥接（禁止全局 event，架构 §9-6）。
 * 泛型 TChunk 默认 StreamChunk；变式生成等场景为 VariantDraft。
 */
export async function invokeStream<TChunk = StreamChunk>(
  cmd: string,
  args: Record<string, unknown>,
  onChunk: (chunk: TChunk) => void
): Promise<void> {
  const channel = new Channel<TChunk>();
  channel.onmessage = onChunk;
  await call<void>(cmd, { ...args, channel });
}
