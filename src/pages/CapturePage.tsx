// 拍照/图片录入页：选图 → OpenCV 圈选校正 → 创建错题（自动触发 OCR+归因流水线）
import React, { useState } from "react";
import { Box, Button, Paper, Stack, Typography } from "@mui/material";
import PhotoCameraIcon from "@mui/icons-material/PhotoCamera";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { appDataDir, join } from "@tauri-apps/api/path";
import { mkdir, writeFile } from "@tauri-apps/plugin-fs";
import ImageCropper, { type CropResult } from "../components/ImageCropper";
import { mistakeApi } from "../api/mistake";
import { useAppStore } from "../stores/appStore";
import { dataUrlToUint8Array } from "../utils/download";

type Step = "pick" | "crop" | "done";

export default function CapturePage(): React.JSX.Element {
  const navigate = useNavigate();
  const notify = useAppStore((s) => s.notify);
  const [step, setStep] = useState<Step>("pick");
  const [imageSrc, setImageSrc] = useState<string>("");
  const [sourcePath, setSourcePath] = useState<string>("");
  const [creating, setCreating] = useState(false);

  /** 选择图片文件（桌面端经 dialog 拿绝对路径） */
  const pickImage = async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp"] }],
    });
    if (!selected || typeof selected !== "string") return;
    setSourcePath(selected);
    // 经 asset 协议加载到裁剪器
    const { convertFileSrc } = await import("@tauri-apps/api/core");
    setImageSrc(convertFileSrc(selected));
    setStep("crop");
  };

  /** 圈选/校正确认 → 创建错题 */
  const handleConfirm = async (result: CropResult) => {
    setCreating(true);
    try {
      let path = sourcePath;
      let region = result.region ?? null;

      if (!region) {
        // 透视校正产物是 dataURL：写入临时文件后走同一入口
        const dir = await appDataDir();
        const imagesDir = await join(dir, "images");
        await mkdir(imagesDir, { recursive: true });
        const tmpPath = await join(imagesDir, `corrected-${Date.now()}.png`);
        await writeFile(tmpPath, dataUrlToUint8Array(result.dataUrl));
        path = tmpPath;
        region = null;
      }

      const mistake = await mistakeApi.createFromImage(path, region ?? undefined);
      notify({
        severity: "success",
        message: `错题 #${mistake.id} 已录入，OCR 与归因正在后台执行`,
      });
      navigate(`/mistakes/${mistake.id}`);
    } catch {
      // 统一弹错已由 api client 处理
    } finally {
      setCreating(false);
    }
  };

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 700 }}>
        拍照录入
      </Typography>
      {step === "pick" && (
        <Paper sx={{ p: 6, textAlign: "center" }}>
          <PhotoCameraIcon sx={{ fontSize: 56, color: "primary.main" }} />
          <Typography variant="body1" sx={{ mt: 2 }}>
            选择一张错题照片，系统将自动检测边缘、圈选题目区域
          </Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
            建议：正面拍摄、光线均匀、题目完整入镜
          </Typography>
          <Stack direction="row" spacing={2} justifyContent="center" sx={{ mt: 3 }}>
            <Button variant="contained" size="large" onClick={pickImage}>
              选择图片
            </Button>
            <Button size="large" onClick={() => navigate("/import")}>
              批量导入
            </Button>
          </Stack>
        </Paper>
      )}
      {step === "crop" && (
        <Paper sx={{ p: 2 }}>
          <ImageCropper
            imageSrc={imageSrc}
            onConfirm={handleConfirm}
            onCancel={() => setStep("pick")}
          />
          {creating && (
            <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
              正在创建错题…
            </Typography>
          )}
        </Paper>
      )}
    </Box>
  );
}
