// 诊断报告 API（F-13）
import { call } from "./client";
import type { ReportDto } from "../types";

export const masteryApi = {
  report: (rangeDays = 30) =>
    call<ReportDto>("mastery_report", { profileId: 1, rangeDays }),
};
