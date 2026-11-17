// 复习计划 API（F-14）
import { call } from "./client";
import type { ReviewItem, ReviewOutcome, ReviewPlan } from "../types";

export const reviewApi = {
  today: () => call<ReviewItem[]>("review_today", { profileId: 1 }),
  complete: (planId: number, isCorrect: boolean) =>
    call<ReviewOutcome>("review_complete", { planId, isCorrect }),
  upcoming: (days = 60) =>
    call<ReviewPlan[]>("review_upcoming", { profileId: 1, days }),
  getIntervals: () => call<number[]>("review_config_get", { profileId: 1 }),
  setIntervals: (intervals: number[]) =>
    call<void>("review_config_set", { intervals, profileId: 1 }),
};
