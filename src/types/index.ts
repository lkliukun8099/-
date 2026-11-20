// 全部 TS 类型定义（与 Rust DTO 镜像，serde camelCase，架构 §4.4 / §9-3）

// ---- 通用 ----

export interface AppError {
  code: number;
  message: string;
  detail?: string;
}

export interface Page<T> {
  items: T[];
  total: number;
  page: number;
  pageSize: number;
}

export interface StreamChunk {
  delta: string;
  done: boolean;
  usage?: { tokensIn: number; tokensOut: number };
  degraded?: string;
}

// ---- 错题 ----

export type MistakeStatus =
  | "pending_analysis"
  | "analyzing"
  | "analyzed"
  | "conflict"
  | "archived";

export interface Mistake {
  id: number;
  profileId: number;
  subjectId: number | null;
  subjectName: string | null;
  sourceImage: string;
  processedImage: string | null;
  ocrText: string | null;
  ocrLatex: string | null;
  ocrEngine: string | null;
  ocrConfidence: number | null;
  studentAnswer: string | null;
  correctAnswer: string | null;
  chapter: string | null;
  questionType: string | null;
  difficulty: number;
  source: string | null;
  status: MistakeStatus;
  version: number;
  createdAt: string;
  updatedAt: string;
}

export interface MistakeSummary {
  id: number;
  subjectId: number | null;
  subjectName: string | null;
  ocrText: string | null;
  status: MistakeStatus;
  chapter: string | null;
  questionType: string | null;
  difficulty: number;
  sourceImage: string;
  tags: string[];
  createdAt: string;
}

export interface MistakeQuery {
  subjectId?: number | null;
  status?: string | null;
  tagId?: number | null;
  keyword?: string | null;
  dateFrom?: string | null;
  dateTo?: string | null;
  page: number;
  pageSize: number;
}

export interface MistakePatch {
  subjectId?: number | null;
  ocrText?: string | null;
  ocrLatex?: string | null;
  studentAnswer?: string | null;
  correctAnswer?: string | null;
  chapter?: string | null;
  questionType?: string | null;
  difficulty?: number | null;
  source?: string | null;
  status?: string | null;
  version: number;
}

export interface MistakeDetail {
  mistake: Mistake;
  attributions: Attribution[];
  variants: VariantQuestion[];
  tags: Tag[];
  reviewPlans: ReviewPlan[];
}

export interface Tag {
  id: number;
  name: string;
  color: string | null;
}

export interface ImageRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

// ---- OCR ----

export interface OcrResult {
  text: string;
  latex: string | null;
  engine: string;
  confidence: number;
  consistency: number | null;
  latencyMs: number;
}

// ---- 归因与冲突 ----

export interface Attribution {
  id: number;
  mistakeId: number;
  origin: "local" | "cloud" | "user";
  engine: string;
  causeLabel: string;
  causeDetail: string | null;
  kgNodeId: number | null;
  kgPath: string[];
  confidence: number;
  isActive: boolean;
  supersededBy: number | null;
  createdAt: string;
}

export interface AttributionConfirm {
  attributionId: number;
  editedLabel?: string;
  editedDetail?: string;
  editedKgNodeId?: number;
}

export interface Conflict {
  id: number;
  entityType: string;
  entityId: number;
  localAttr: Attribution;
  cloudAttr: Attribution;
  status: "pending" | "resolved" | "ignored";
  resolvedChoice: string | null;
  createdAt: string;
}

// ---- 变式 ----

export interface VariantQuestion {
  id: number;
  mistakeId: number;
  kgNodeId: number | null;
  content: string;
  contentLatex: string | null;
  answer: string;
  solution: string | null;
  difficulty: number;
  variantType: "isomorphic" | "recontext" | "advanced" | string;
  generatorModel: string | null;
  validateStatus: "pending" | "passed" | "failed" | "manual_review";
  validateDetail: string | null;
  createdAt: string;
}

export interface VariantDraft {
  content: string;
  contentLatex: string | null;
  answer: string;
  solution: string | null;
  difficulty: number;
  variantType: string;
  validateStatus: "pending" | "passed" | "failed" | "manual_review";
  variantId: number | null;
}

export interface AttemptFeedback {
  isCorrect: boolean;
  feedback: string;
  masteryDelta: number | null;
}

// ---- 知识图谱 ----

export interface KgNode {
  id: number;
  subjectId: number;
  parentId: number | null;
  code: string;
  name: string;
  level: number;
  textbook: string;
  extra: string | null;
}

export type KgTreeNode = KgNode & { children: KgTreeNode[] };

export interface KgTree {
  subjectId: number;
  roots: KgTreeNode[];
}

// ---- 掌握度 / 报告 ----

export interface HeatCell {
  kgNodeId: number;
  nodeName: string;
  nodePath: string[];
  score: number;
  attempts: number;
}

export interface CurvePoint {
  date: string;
  avgScore: number;
}

export interface CauseCount {
  causeLabel: string;
  count: number;
}

export interface ReportDto {
  heatmap: HeatCell[];
  curve: CurvePoint[];
  causeDistribution: CauseCount[];
  totalMistakes: number;
  reviewedToday: number;
}

// ---- 复习 ----

export interface ReviewPlan {
  id: number;
  profileId: number;
  targetType: "mistake" | "variant";
  targetId: number;
  stage: number;
  nextDueAt: string;
  status: "pending" | "done" | "skipped" | "expired";
  completedAt: string | null;
  createdAt: string;
}

export interface ReviewItem {
  plan: ReviewPlan;
  title: string;
  subjectName: string | null;
}

export interface ReviewOutcome {
  hasNext: boolean;
  nextPlan: ReviewPlan | null;
  stage: number;
}

// ---- 模型接入 ----

export interface ModelSpec {
  id: string;
  priceIn: number;
  priceOut: number;
  ctx: number;
}

export interface ModelProvider {
  id: number;
  name: string;
  protocol: string;
  baseUrl: string;
  keyAlias: string;
  models: ModelSpec[];
  enabled: boolean;
  priority: number;
  hasKey: boolean;
}

export interface ProviderInput {
  id?: number;
  name: string;
  protocol?: string;
  baseUrl: string;
  apiKey?: string;
  models: ModelSpec[];
  enabled?: boolean;
  priority?: number;
}

export interface RoutePolicy {
  id: number;
  taskType: string;
  primary: string;
  fallbacks: string[];
  timeoutMs: number;
  maxCostCny: number | null;
  monthlyBudgetCny: number | null;
}

export interface ChatMessage {
  role: string;
  content: string;
}

export interface CallLog {
  id: number;
  provider: string;
  model: string;
  taskType: string;
  tokensIn: number;
  tokensOut: number;
  costCny: number;
  latencyMs: number | null;
  status: string;
  errorCode: string | null;
  requestHash: string | null;
  createdAt: string;
}

export interface CostByProvider {
  provider: string;
  costCny: number;
  calls: number;
}

export interface CostByDay {
  date: string;
  costCny: number;
  calls: number;
}

export interface CostReport {
  totalCostCny: number;
  totalCalls: number;
  totalTokensIn: number;
  totalTokensOut: number;
  byProvider: CostByProvider[];
  byDay: CostByDay[];
  recentLogs: CallLog[];
}

// ---- 系统 ----

export interface BackupInfo {
  path: string;
  name: string;
  sizeBytes: number;
  createdAt: string;
}

/** 学科常量（迁移 V1 固定写入顺序） */
export const SUBJECTS = [
  { id: 1, code: "math", name: "数学" },
  { id: 2, code: "physics", name: "物理" },
  { id: 3, code: "chemistry", name: "化学" },
  { id: 4, code: "english", name: "英语" },
] as const;

// ---- 对话式错题分析（聊天入口） ----
export interface ChatAnalyzeResult {
  mistakeId: number;
  attribution: Attribution | null;
  variants: VariantDraft[];
  note: string | null;
}
