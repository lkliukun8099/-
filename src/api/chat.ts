// 对话式错题分析 API（聊天入口：文本建题→归因→变式，一次调用）
import { call } from "./client";
import type { ChatAnalyzeResult } from "../types";

export const chatApi = {
  analyze: (question: string, myAnswer?: string) =>
    call<ChatAnalyzeResult>("chat_analyze", {
      question,
      myAnswer: myAnswer ?? null,
    }),
};
