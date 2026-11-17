// 归因与冲突合并 API（F-06/F-16）
import { call } from "./client";
import type { Attribution, AttributionConfirm, Conflict } from "../types";

export type AttributionMode = "local" | "cloud" | "auto";

export const attributionApi = {
  run: (mistakeId: number, mode: AttributionMode = "auto") =>
    call<Attribution>("attribution_run", { mistakeId, mode }),
  confirm: (input: AttributionConfirm) =>
    call<void>("attribution_confirm", { input }),
  conflictList: (status?: string) =>
    call<Conflict[]>("conflict_list", { status: status ?? null }),
  conflictResolve: (
    conflictId: number,
    choice: "local" | "cloud" | "merged",
    mergedLabel?: string,
    mergedDetail?: string
  ) =>
    call<void>("conflict_resolve", {
      conflictId,
      choice,
      mergedLabel: mergedLabel ?? null,
      mergedDetail: mergedDetail ?? null,
    }),
};
