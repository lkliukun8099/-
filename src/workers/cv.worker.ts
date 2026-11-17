/**
 * OpenCV.js WASM 图像处理 Worker（F-01：边缘检测/透视校正/圈选裁切）。
 *
 * 消息协议：
 * 请求  { id, action, image: ImageData, points?: Point[], region?: Region }
 * 响应  { id, ok, payload?, error? }
 *
 * action:
 * - detectEdges         → payload: { points: Point[4] }（归一化，tl/tr/br/bl）
 * - perspectiveCorrect  → payload: { image: ImageData }（按 4 点拉正）
 * - cropRegion          → payload: { image: ImageData }（按归一化矩形裁切）
 */
import cvModule from "@techstark/opencv-js";

interface Point {
  x: number;
  y: number;
}

interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface WorkerRequest {
  id: number;
  action: "detectEdges" | "perspectiveCorrect" | "cropRegion";
  image: ImageData;
  points?: Point[];
  region?: Region;
}

// ---- OpenCV 初始化（兼容 Promise / onRuntimeInitialized 两种导出形态）----
let cvReady: Promise<any> | null = null;

function getCv(): Promise<any> {
  if (!cvReady) {
    cvReady = new Promise((resolve) => {
      const mod: any = cvModule as any;
      if (mod && typeof mod.then === "function") {
        mod.then((cv: any) => resolve(cv));
      } else if (mod && mod.Mat) {
        resolve(mod);
      } else if (mod) {
        mod.onRuntimeInitialized = () => resolve(mod);
      } else {
        throw new Error("OpenCV 模块加载失败");
      }
    });
  }
  return cvReady;
}

/** 四角点排序：左上、右上、右下、左下 */
function orderPoints(pts: Point[]): Point[] {
  const sorted = [...pts].sort((a, b) => a.y - b.y);
  const top = sorted.slice(0, 2).sort((a, b) => a.x - b.x);
  const bottom = sorted.slice(2, 4).sort((a, b) => b.x - a.x);
  return [top[0], top[1], bottom[0], bottom[1]];
}

/** 边缘检测：找最大四边形轮廓，返回归一化四角点 */
function detectEdges(cv: any, image: ImageData): Point[] {
  const src = cv.matFromImageData(image);
  const maxDim = 1200;
  const scale = Math.min(1, maxDim / Math.max(src.cols, src.rows));
  const work = new cv.Mat();
  if (scale < 1) {
    cv.resize(src, work, new cv.Size(0, 0), scale, scale, cv.INTER_AREA);
  } else {
    work.copyFrom ? work.copyFrom(src) : src.copyTo(work);
  }

  const gray = new cv.Mat();
  cv.cvtColor(work, gray, cv.COLOR_RGBA2GRAY);
  cv.GaussianBlur(gray, gray, new cv.Size(5, 5), 0);
  const edges = new cv.Mat();
  cv.Canny(gray, edges, 50, 150);
  const kernel = cv.Mat.ones(3, 3, cv.CV_8U);
  cv.dilate(edges, edges, kernel);

  const contours = new cv.MatVector();
  const hierarchy = new cv.Mat();
  cv.findContours(edges, contours, hierarchy, cv.RETR_LIST, cv.CHAIN_APPROX_SIMPLE);

  let best: Point[] | null = null;
  let bestArea = 0;
  for (let i = 0; i < contours.size(); i++) {
    const cnt = contours.get(i);
    const peri = cv.arcLength(cnt, true);
    const approx = new cv.Mat();
    cv.approxPolyDP(cnt, approx, 0.02 * peri, true);
    if (approx.rows === 4) {
      const area = cv.contourArea(approx);
      if (area > bestArea) {
        bestArea = area;
        const pts: Point[] = [];
        for (let j = 0; j < 4; j++) {
          pts.push({
            x: approx.intPtr(j, 0)[0] / work.cols,
            y: approx.intPtr(j, 0)[1] / work.rows,
          });
        }
        best = pts;
      }
    }
    cnt.delete();
    approx.delete();
  }

  [src, work, gray, edges, kernel, contours, hierarchy].forEach((m: any) => m.delete());

  // 未检测到四边形时回退整图（留 2% 边距）
  if (!best) {
    best = [
      { x: 0.02, y: 0.02 },
      { x: 0.98, y: 0.02 },
      { x: 0.98, y: 0.98 },
      { x: 0.02, y: 0.98 },
    ];
  }
  return orderPoints(best);
}

/** 透视校正：按四角点拉正为矩形 */
function perspectiveCorrect(cv: any, image: ImageData, points: Point[]): ImageData {
  const src = cv.matFromImageData(image);
  const w = image.width;
  const h = image.height;
  const pts = orderPoints(points).map((p) => [p.x * w, p.y * h]);

  const widthTop = Math.hypot(pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]);
  const widthBottom = Math.hypot(pts[2][0] - pts[3][0], pts[2][1] - pts[3][1]);
  const heightLeft = Math.hypot(pts[3][0] - pts[0][0], pts[3][1] - pts[0][1]);
  const heightRight = Math.hypot(pts[2][0] - pts[1][0], pts[2][1] - pts[1][1]);
  const outW = Math.max(1, Math.round(Math.max(widthTop, widthBottom)));
  const outH = Math.max(1, Math.round(Math.max(heightLeft, heightRight)));

  const srcTri = cv.matFromArray(4, 1, cv.CV_32FC2, [
    pts[0][0], pts[0][1],
    pts[1][0], pts[1][1],
    pts[2][0], pts[2][1],
    pts[3][0], pts[3][1],
  ]);
  const dstTri = cv.matFromArray(4, 1, cv.CV_32FC2, [
    0, 0, outW - 1, 0, outW - 1, outH - 1, 0, outH - 1,
  ]);
  const m = cv.getPerspectiveTransform(srcTri, dstTri);
  const dst = new cv.Mat();
  cv.warpPerspective(src, dst, m, new cv.Size(outW, outH), cv.INTER_LINEAR, cv.BORDER_CONSTANT);

  const out = new ImageData(new Uint8ClampedArray(dst.data), outW, outH);
  [src, srcTri, dstTri, m, dst].forEach((x: any) => x.delete());
  return out;
}

/** 归一化矩形裁切 */
function cropRegion(cv: any, image: ImageData, region: Region): ImageData {
  const src = cv.matFromImageData(image);
  const x = Math.max(0, Math.round(region.x * src.cols));
  const y = Math.max(0, Math.round(region.y * src.rows));
  const rw = Math.min(src.cols - x, Math.max(1, Math.round(region.width * src.cols)));
  const rh = Math.min(src.rows - y, Math.max(1, Math.round(region.height * src.rows)));
  const rect = new cv.Rect(x, y, rw, rh);
  const cropped = src.roi(rect);
  const out = new ImageData(new Uint8ClampedArray(cropped.data), rw, rh);
  src.delete();
  cropped.delete();
  return out;
}

self.onmessage = async (e: MessageEvent<WorkerRequest>) => {
  const { id, action, image, points, region } = e.data;
  try {
    const cv = await getCv();
    let payload: unknown;
    switch (action) {
      case "detectEdges":
        payload = { points: detectEdges(cv, image) };
        break;
      case "perspectiveCorrect":
        if (!points || points.length !== 4) throw new Error("需要 4 个角点");
        payload = { image: perspectiveCorrect(cv, image, points) };
        break;
      case "cropRegion":
        if (!region) throw new Error("缺少裁切区域");
        payload = { image: cropRegion(cv, image, region) };
        break;
      default:
        throw new Error(`未知动作: ${action}`);
    }
    (self as unknown as Worker).postMessage({ id, ok: true, payload });
  } catch (err) {
    (self as unknown as Worker).postMessage({
      id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
};

export {};
