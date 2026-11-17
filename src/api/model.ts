// 模型接入 API（F-10/11/19）
import { call, invokeStream } from "./client";
import type {
  ChatMessage,
  CostReport,
  ModelProvider,
  ProviderInput,
  RoutePolicy,
  StreamChunk,
} from "../types";

export const modelApi = {
  providerList: () => call<ModelProvider[]>("model_provider_list"),
  providerUpsert: (input: ProviderInput) =>
    call<number>("model_provider_upsert", { input }),
  providerTest: (providerId: number) =>
    call<string>("model_provider_test", { providerId }),
  providerDelete: (providerId: number) =>
    call<void>("model_provider_delete", { providerId }),
  keyRotate: (keyAlias: string, apiKey: string) =>
    call<number>("model_key_rotate", { keyAlias, apiKey }),
  policyList: () => call<RoutePolicy[]>("route_policy_list"),
  policyUpsert: (policy: RoutePolicy) =>
    call<number>("route_policy_upsert", { policy }),
  /** 统一聊天入口（流式） */
  aiChat: (
    taskType: string,
    messages: ChatMessage[],
    onChunk: (chunk: StreamChunk) => void
  ) => invokeStream<StreamChunk>("ai_chat", { taskType, messages }, onChunk),
  costStats: (rangeDays = 30) =>
    call<CostReport>("cost_stats", { rangeDays }),
};
