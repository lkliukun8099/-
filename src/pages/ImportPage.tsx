// 相册/文件批量导入页（F-02；PDF 整卷拆题为 P1 预留，U-6）
import React, { useState } from "react";
import {
  Box,
  Button,
  Chip,
  LinearProgress,
  List,
  ListItem,
  ListItemText,
  Paper,
  Typography,
} from "@mui/material";
import UploadFileIcon from "@mui/icons-material/UploadFile";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { mistakeApi } from "../api/mistake";
import { useAppStore } from "../stores/appStore";

interface ImportItem {
  path: string;
  status: "pending" | "ok" | "failed";
  mistakeId?: number;
  error?: string;
}

export default function ImportPage(): React.JSX.Element {
  const navigate = useNavigate();
  const notify = useAppStore((s) => s.notify);
  const [items, setItems] = useState<ImportItem[]>([]);
  const [importing, setImporting] = useState(false);

  const pickFiles = async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp"] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    setItems(paths.map((p) => ({ path: p, status: "pending" as const })));
  };

  const startImport = async () => {
    setImporting(true);
    let okCount = 0;
    for (const item of items) {
      if (item.status === "ok") continue;
      try {
        const mistake = await mistakeApi.createFromImage(item.path);
        okCount++;
        setItems((prev) =>
          prev.map((it) =>
            it.path === item.path ? { ...it, status: "ok", mistakeId: mistake.id } : it
          )
        );
      } catch (e) {
        setItems((prev) =>
          prev.map((it) =>
            it.path === item.path
              ? { ...it, status: "failed", error: e instanceof Error ? e.message : String(e) }
              : it
          )
        );
      }
    }
    setImporting(false);
    notify({ severity: "success", message: `导入完成：成功 ${okCount} 张` });
  };

  const doneCount = items.filter((i) => i.status === "ok").length;

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 700 }}>
        相册 / 文件导入
      </Typography>
      <Paper sx={{ p: 3 }}>
        <Button variant="contained" startIcon={<UploadFileIcon />} onClick={pickFiles} disabled={importing}>
          选择图片（可多选）
        </Button>
        <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
          PDF 扫描件：请先将页面导出为图片后导入（整卷自动拆题为后续版本能力）。
        </Typography>
        {items.length > 0 && (
          <>
            <Box sx={{ mt: 2 }}>
              <LinearProgress
                variant="determinate"
                value={items.length ? (doneCount / items.length) * 100 : 0}
              />
              <Typography variant="caption">
                {doneCount}/{items.length}
              </Typography>
            </Box>
            <List dense>
              {items.map((item) => (
                <ListItem
                  key={item.path}
                  secondaryAction={
                    item.status === "ok" ? (
                      <Chip
                        size="small"
                        color="success"
                        label={`#${item.mistakeId}`}
                        onClick={() => navigate(`/mistakes/${item.mistakeId}`)}
                      />
                    ) : item.status === "failed" ? (
                      <Chip size="small" color="error" label="失败" />
                    ) : (
                      <Chip size="small" label="待导入" />
                    )
                  }
                >
                  <ListItemText
                    primary={item.path.split(/[\\/]/).pop()}
                    secondary={item.error ?? item.path}
                  />
                </ListItem>
              ))}
            </List>
            <Button
              variant="contained"
              sx={{ mt: 1 }}
              disabled={importing || doneCount === items.length}
              onClick={startImport}
            >
              {importing ? "导入中…" : "开始导入"}
            </Button>
          </>
        )}
      </Paper>
    </Box>
  );
}
