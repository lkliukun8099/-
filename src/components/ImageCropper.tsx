// OpenCV 圈选与四角拖拽校正组件（F-01）。
// 能力：自动边缘检测 → 四角点拖拽微调 → 透视校正 / 手动矩形裁切 / 使用原图。
import React, { useCallback, useEffect, useRef, useState } from "react";
import { Box, Button, CircularProgress, Stack, Typography } from "@mui/material";

export interface CropResult {
  /** 处理后图片 dataURL（PNG） */
  dataUrl: string;
  /** 手动裁切时的归一化区域 */
  region?: { x: number; y: number; width: number; height: number };
}

interface ImageCropperProps {
  imageSrc: string;
  onConfirm: (result: CropResult) => void;
  onCancel?: () => void;
}

interface Point {
  x: number;
  y: number;
}

type CvAction = "detectEdges" | "perspectiveCorrect" | "cropRegion";

/** 调用 cv worker 并等待结果 */
function callWorker<T>(
  worker: Worker,
  action: CvAction,
  image: ImageData,
  extra: Record<string, unknown> = {}
): Promise<T> {
  const id = Math.floor(Math.random() * 1e9);
  return new Promise((resolve, reject) => {
    const handler = (e: MessageEvent) => {
      const msg = e.data;
      if (msg.id !== id) return;
      worker.removeEventListener("message", handler);
      if (msg.ok) resolve(msg.payload as T);
      else reject(new Error(msg.error ?? "图像处理失败"));
    };
    worker.addEventListener("message", handler);
    worker.postMessage({ id, action, image, ...extra });
  });
}

const MAX_DISPLAY_W = 860;

export default function ImageCropper({
  imageSrc,
  onConfirm,
  onCancel,
}: ImageCropperProps): React.JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const workerRef = useRef<Worker | null>(null);
  const imageDataRef = useRef<ImageData | null>(null);
  const [loading, setLoading] = useState(true);
  const [processing, setProcessing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [points, setPoints] = useState<Point[]>([]);
  const [dragIdx, setDragIdx] = useState<number>(-1);
  // 手动矩形框选状态
  const [rectStart, setRectStart] = useState<Point | null>(null);

  /** 重绘画布（底图 + 多边形 + 角点） */
  const redraw = useCallback((pts: Point[]) => {
    const canvas = canvasRef.current;
    const imageData = imageDataRef.current;
    if (!canvas || !imageData) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.putImageData(imageData, 0, 0);
    if (pts.length === 4) {
      ctx.strokeStyle = "#22c55e";
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(pts[0].x * canvas.width, pts[0].y * canvas.height);
      for (let i = 1; i < 4; i++) {
        ctx.lineTo(pts[i].x * canvas.width, pts[i].y * canvas.height);
      }
      ctx.closePath();
      ctx.stroke();
      pts.forEach((p) => {
        ctx.fillStyle = "#f97316";
        ctx.beginPath();
        ctx.arc(p.x * canvas.width, p.y * canvas.height, 7, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = "#fff";
        ctx.stroke();
      });
    }
  }, []);

  // 初始化：加载图片 → 启动 worker → 自动边缘检测
  useEffect(() => {
    let cancelled = false;
    const worker = new Worker(new URL("../workers/cv.worker.ts", import.meta.url), {
      type: "module",
    });
    workerRef.current = worker;

    const img = new Image();
    img.onload = async () => {
      if (cancelled) return;
      const scale = Math.min(1, MAX_DISPLAY_W / img.naturalWidth);
      const w = Math.round(img.naturalWidth * scale);
      const h = Math.round(img.naturalHeight * scale);
      const canvas = canvasRef.current;
      if (!canvas) return;
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.drawImage(img, 0, 0, w, h);
      const imageData = ctx.getImageData(0, 0, w, h);
      imageDataRef.current = imageData;
      try {
        const result = await callWorker<{ points: Point[] }>(worker, "detectEdges", imageData);
        if (cancelled) return;
        setPoints(result.points);
        redraw(result.points);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : "边缘检测失败");
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    img.onerror = () => {
      setError("图片加载失败");
      setLoading(false);
    };
    img.src = imageSrc;

    return () => {
      cancelled = true;
      worker.terminate();
      workerRef.current = null;
    };
  }, [imageSrc, redraw]);

  /** 画布坐标（归一化） */
  const toNormalized = (e: React.MouseEvent<HTMLCanvasElement>): Point => {
    const rect = e.currentTarget.getBoundingClientRect();
    return {
      x: (e.clientX - rect.left) / rect.width,
      y: (e.clientY - rect.top) / rect.height,
    };
  };

  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const p = toNormalized(e);
    // 命中角点则拖拽角点，否则开始矩形框选
    const idx = points.findIndex(
      (pt) => Math.hypot(pt.x - p.x, pt.y - p.y) < 0.03
    );
    if (idx >= 0) {
      setDragIdx(idx);
    } else {
      setRectStart(p);
    }
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const p = toNormalized(e);
    if (dragIdx >= 0) {
      const next = [...points];
      next[dragIdx] = { x: Math.min(1, Math.max(0, p.x)), y: Math.min(1, Math.max(0, p.y)) };
      setPoints(next);
      redraw(next);
    } else if (rectStart) {
      // 矩形预览
      redraw(points);
      const canvas = canvasRef.current;
      const ctx = canvas?.getContext("2d");
      if (canvas && ctx) {
        ctx.strokeStyle = "#0ea5e9";
        ctx.setLineDash([6, 4]);
        ctx.strokeRect(
          rectStart.x * canvas.width,
          rectStart.y * canvas.height,
          (p.x - rectStart.x) * canvas.width,
          (p.y - rectStart.y) * canvas.height
        );
        ctx.setLineDash([]);
      }
    }
  };

  const handleMouseUp = async (e: React.MouseEvent<HTMLCanvasElement>) => {
    setDragIdx(-1);
    if (!rectStart) return;
    const end = toNormalized(e);
    const region = {
      x: Math.min(rectStart.x, end.x),
      y: Math.min(rectStart.y, end.y),
      width: Math.abs(end.x - rectStart.x),
      height: Math.abs(end.y - rectStart.y),
    };
    setRectStart(null);
    if (region.width < 0.03 || region.height < 0.03) return; // 误触忽略
    await doCrop(region);
  };

  /** 执行矩形裁切 */
  const doCrop = async (region: { x: number; y: number; width: number; height: number }) => {
    const worker = workerRef.current;
    const imageData = imageDataRef.current;
    if (!worker || !imageData) return;
    setProcessing(true);
    try {
      const { image } = await callWorker<{ image: ImageData }>(worker, "cropRegion", imageData, {
        region,
      });
      onConfirm({ dataUrl: imageDataToDataUrl(image), region });
    } catch (e2) {
      setError(e2 instanceof Error ? e2.message : "裁切失败");
    } finally {
      setProcessing(false);
    }
  };

  /** 透视校正 */
  const doCorrect = async () => {
    const worker = workerRef.current;
    const imageData = imageDataRef.current;
    if (!worker || !imageData || points.length !== 4) return;
    setProcessing(true);
    try {
      const { image } = await callWorker<{ image: ImageData }>(
        worker,
        "perspectiveCorrect",
        imageData,
        { points }
      );
      onConfirm({ dataUrl: imageDataToDataUrl(image) });
    } catch (e2) {
      setError(e2 instanceof Error ? e2.message : "透视校正失败");
    } finally {
      setProcessing(false);
    }
  };

  /** 直接使用原图 */
  const useOriginal = () => {
    const imageData = imageDataRef.current;
    if (!imageData) return;
    onConfirm({ dataUrl: imageDataToDataUrl(imageData) });
  };

  return (
    <Box>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
        拖动四角点调整题目区域后点「透视校正」；或在图上拖出矩形框选后自动裁切。
      </Typography>
      <Box sx={{ position: "relative", display: "inline-block", maxWidth: "100%" }}>
        <canvas
          ref={canvasRef}
          style={{ maxWidth: "100%", cursor: dragIdx >= 0 ? "grabbing" : "crosshair" }}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
        />
        {(loading || processing) && (
          <Box
            sx={{
              position: "absolute",
              inset: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              bgcolor: "rgba(255,255,255,0.6)",
            }}
          >
            <CircularProgress size={36} />
          </Box>
        )}
      </Box>
      {error && (
        <Typography color="error" variant="body2" sx={{ mt: 1 }}>
          {error}
        </Typography>
      )}
      <Stack direction="row" spacing={1} sx={{ mt: 2 }}>
        <Button
          variant="contained"
          disabled={loading || processing || points.length !== 4}
          onClick={doCorrect}
        >
          透视校正
        </Button>
        <Button variant="outlined" disabled={loading || processing} onClick={useOriginal}>
          使用原图
        </Button>
        {onCancel && (
          <Button variant="text" onClick={onCancel}>
            取消
          </Button>
        )}
      </Stack>
    </Box>
  );
}

/** ImageData → PNG dataURL */
function imageDataToDataUrl(image: ImageData): string {
  const canvas = document.createElement("canvas");
  canvas.width = image.width;
  canvas.height = image.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Canvas 初始化失败");
  ctx.putImageData(image, 0, 0);
  return canvas.toDataURL("image/png");
}
