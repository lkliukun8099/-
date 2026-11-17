// api 层 invoke 参数契约测试（架构 §9-3：Command 参数 camelCase）
// 逐条验证前端发送的命令名与参数键名，防止与 Rust 侧 serde/rename_all 漂移。
import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(null),
  Channel: class MockChannel {
    onmessage: ((c: unknown) => void) | null = null;
  },
}));

import { invoke } from "@tauri-apps/api/core";
import { mistakeApi } from "../src/api/mistake";
import { ocrApi } from "../src/api/ocr";
import { attributionApi } from "../src/api/attribution";
import { variantApi } from "../src/api/variant";
import { kgApi } from "../src/api/kg";
import { masteryApi } from "../src/api/mastery";
import { reviewApi } from "../src/api/review";
import { modelApi } from "../src/api/model";
import { settingsApi } from "../src/api/settings";

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockedInvoke.mockClear();
  mockedInvoke.mockResolvedValue(null as never);
});

/** 断言最后一次 invoke 的命令名与参数键集合 */
function expectCall(cmd: string, argKeys: string[] | null) {
  expect(mockedInvoke).toHaveBeenCalled();
  const [actualCmd, args] = mockedInvoke.mock.lastCall!;
  expect(actualCmd).toBe(cmd);
  if (argKeys === null) {
    expect(args).toBeUndefined();
  } else {
    expect(Object.keys((args ?? {}) as object).sort()).toEqual(argKeys.sort());
  }
}

describe("错题命令契约", () => {
  it("mistake_create_from_image: {imagePath, region}", async () => {
    await mistakeApi.createFromImage("C:/tmp/a.png", { x: 0, y: 0, width: 1, height: 1 });
    expectCall("mistake_create_from_image", ["imagePath", "region"]);
  });
  it("mistake_list: {query}", async () => {
    await mistakeApi.list({ page: 1, pageSize: 20, keyword: "函数" });
    expectCall("mistake_list", ["query"]);
  });
  it("mistake_get/update/delete", async () => {
    await mistakeApi.get(1);
    expectCall("mistake_get", ["id"]);
    await mistakeApi.update(1, { version: 2, chapter: "第一章" });
    expectCall("mistake_update", ["id", "patch"]);
    await mistakeApi.delete(1);
    expectCall("mistake_delete", ["id"]);
  });
  it("tag_list / tag_create / mistake_set_tags", async () => {
    await mistakeApi.tagList();
    expectCall("tag_list", null);
    await mistakeApi.tagCreate("易错", "#f00");
    expectCall("tag_create", ["name", "color"]);
    await mistakeApi.setTags(1, [2, 3]);
    expectCall("mistake_set_tags", ["mistakeId", "tagIds"]);
  });
});

describe("OCR / 归因 / 冲突命令契约", () => {
  it("ocr_run: {mistakeId, prefer}", async () => {
    await ocrApi.run(5, "offline");
    expectCall("ocr_run", ["mistakeId", "prefer"]);
  });
  it("attribution_run: {mistakeId, mode}", async () => {
    await attributionApi.run(5, "cloud");
    expectCall("attribution_run", ["mistakeId", "mode"]);
  });
  it("attribution_confirm: {input}", async () => {
    await attributionApi.confirm({ attributionId: 9, editedLabel: "新标签" });
    expectCall("attribution_confirm", ["input"]);
  });
  it("conflict_list / conflict_resolve（merged 需带 mergedLabel）", async () => {
    await attributionApi.conflictList();
    expectCall("conflict_list", ["status"]);
    await attributionApi.conflictResolve(3, "merged", "合并标签", "合并说明");
    expectCall("conflict_resolve", ["conflictId", "choice", "mergedLabel", "mergedDetail"]);
  });
});

describe("变式 / 复习 / 图谱 / 报告命令契约", () => {
  it("variant_generate 走 Channel：命令参数带 channel 且其余为 camelCase", async () => {
    const chunks: unknown[] = [];
    await variantApi.generate(7, 2, (d) => chunks.push(d));
    const [cmd, args] = mockedInvoke.mock.lastCall!;
    expect(cmd).toBe("variant_generate");
    const keys = Object.keys(args as object).sort();
    expect(keys).toEqual(["channel", "count", "mistakeId"]);
  });
  it("variant_train_queue / variant_submit_attempt", async () => {
    await variantApi.trainQueue(10);
    expectCall("variant_train_queue", ["profileId", "limit"]);
    await variantApi.submitAttempt(3, "a≥2", 1500);
    expectCall("variant_submit_attempt", ["variantId", "answer", "durationMs", "profileId"]);
  });
  it("review_* 系列", async () => {
    await reviewApi.today();
    expectCall("review_today", ["profileId"]);
    await reviewApi.complete(11, true);
    expectCall("review_complete", ["planId", "isCorrect"]);
    await reviewApi.upcoming(30);
    expectCall("review_upcoming", ["profileId", "days"]);
    await reviewApi.getIntervals();
    expectCall("review_config_get", ["profileId"]);
    await reviewApi.setIntervals([1, 3, 7]);
    expectCall("review_config_set", ["intervals", "profileId"]);
  });
  it("kg_tree / kg_search / mastery_report", async () => {
    await kgApi.tree(1);
    expectCall("kg_tree", ["subjectId"]);
    await kgApi.search(1, "函数");
    expectCall("kg_search", ["subjectId", "keyword"]);
    await masteryApi.report(14);
    expectCall("mastery_report", ["profileId", "rangeDays"]);
  });
});

describe("模型接入 / 系统命令契约", () => {
  it("model_provider_* / route_policy_*", async () => {
    await modelApi.providerList();
    expectCall("model_provider_list", null);
    await modelApi.providerUpsert({
      name: "deepseek",
      baseUrl: "https://api.deepseek.com/v1",
      models: [{ id: "deepseek-chat", priceIn: 0.1, priceOut: 0.2, ctx: 64000 }],
    });
    expectCall("model_provider_upsert", ["input"]);
    await modelApi.providerTest(2);
    expectCall("model_provider_test", ["providerId"]);
    await modelApi.providerDelete(2);
    expectCall("model_provider_delete", ["providerId"]);
    await modelApi.keyRotate("provider-deepseek", "sk-xxx");
    expectCall("model_key_rotate", ["keyAlias", "apiKey"]);
    await modelApi.policyList();
    expectCall("route_policy_list", null);
  });
  it("ai_chat 走 Channel 且参数 camelCase", async () => {
    await modelApi.aiChat("long_text", [{ role: "user", content: "hi" }], () => {});
    const [cmd, args] = mockedInvoke.mock.lastCall!;
    expect(cmd).toBe("ai_chat");
    expect(Object.keys(args as object).sort()).toEqual(["channel", "messages", "taskType"]);
  });
  it("cost_stats / settings / backup / sync", async () => {
    await modelApi.costStats(30);
    expectCall("cost_stats", ["rangeDays"]);
    await settingsApi.get("theme");
    expectCall("settings_get", ["key"]);
    await settingsApi.set("theme", "dark");
    expectCall("settings_set", ["key", "value"]);
    await settingsApi.backupRun();
    expectCall("backup_run", null);
    await settingsApi.backupList();
    expectCall("backup_list", null);
    await settingsApi.backupRestore("C:/backups/maiduo-20250101.db");
    expectCall("backup_restore", ["backupPath"]);
    await settingsApi.logClear();
    expectCall("log_clear", null);
    await settingsApi.networkStatus();
    expectCall("network_status", null);
    await settingsApi.syncPush();
    expectCall("sync_push", null);
    await settingsApi.syncPull();
    expectCall("sync_pull", null);
  });
});
