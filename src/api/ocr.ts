// OCR API（F-04）
import { call } from "./client";
import type { OcrResult } from "../types";

export type OcrPrefer = "auto" | "cloud" | "offline";

export const ocrApi = {
  run: (mistakeId: number, prefer: OcrPrefer = "auto") =>
    call<OcrResult>("ocr_run", { mistakeId, prefer }),
};
