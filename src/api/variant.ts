// 变式题 API（F-08/09/15）
import { call, invokeStream } from "./client";
import type { AttemptFeedback, VariantDraft, VariantQuestion } from "../types";

export const variantApi = {
  /** 流式生成：每道变式（含校验状态与入库 ID）逐条回调 */
  generate: (
    mistakeId: number,
    count: number,
    onDraft: (draft: VariantDraft) => void
  ) =>
    invokeStream<VariantDraft>(
      "variant_generate",
      { mistakeId, count },
      onDraft
    ),
  trainQueue: (limit = 20) =>
    call<VariantQuestion[]>("variant_train_queue", {
      profileId: 1,
      limit,
    }),
  submitAttempt: (variantId: number, answer: string, durationMs?: number) =>
    call<AttemptFeedback>("variant_submit_attempt", {
      variantId,
      answer,
      durationMs: durationMs ?? null,
      profileId: 1,
    }),
};
