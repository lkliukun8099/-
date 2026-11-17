// 系统设置 API（N-03/N-04）+ 同步桩（F-21）
import { call } from "./client";
import type { BackupInfo } from "../types";

export const settingsApi = {
  get: (key: string) =>
    call<string | null>("settings_get", { key }, true),
  set: (key: string, value: string) =>
    call<void>("settings_set", { key, value }),
  backupRun: () => call<string>("backup_run"),
  backupList: () => call<BackupInfo[]>("backup_list"),
  backupRestore: (backupPath: string) =>
    call<void>("backup_restore", { backupPath }),
  logClear: () => call<number>("log_clear"),
  networkStatus: () => call<boolean>("network_status", undefined, true),
  syncPush: () => call<void>("sync_push"),
  syncPull: () => call<void>("sync_pull"),
};
