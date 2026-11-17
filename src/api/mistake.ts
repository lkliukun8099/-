// 错题 API（F-01/02/05）
import { call } from "./client";
import type {
  ImageRegion,
  Mistake,
  MistakeDetail,
  MistakePatch,
  MistakeQuery,
  MistakeSummary,
  Page,
  Tag,
} from "../types";

export const mistakeApi = {
  createFromImage: (imagePath: string, region?: ImageRegion) =>
    call<Mistake>("mistake_create_from_image", {
      imagePath,
      region: region ?? null,
    }),
  list: (query: MistakeQuery) =>
    call<Page<MistakeSummary>>("mistake_list", { query }),
  get: (id: number) => call<MistakeDetail>("mistake_get", { id }),
  update: (id: number, patch: MistakePatch) =>
    call<void>("mistake_update", { id, patch }),
  delete: (id: number) => call<void>("mistake_delete", { id }),
  tagList: () => call<Tag[]>("tag_list"),
  tagCreate: (name: string, color?: string) =>
    call<number>("tag_create", { name, color: color ?? null }),
  setTags: (mistakeId: number, tagIds: number[]) =>
    call<void>("mistake_set_tags", { mistakeId, tagIds }),
};
