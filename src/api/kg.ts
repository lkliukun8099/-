// 知识图谱 API（F-07）
import { call } from "./client";
import type { KgNode, KgTree } from "../types";

export const kgApi = {
  tree: (subjectId: number) => call<KgTree>("kg_tree", { subjectId }),
  search: (subjectId: number, keyword: string) =>
    call<KgNode[]>("kg_search", { subjectId, keyword }),
};
