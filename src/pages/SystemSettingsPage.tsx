// 系统设置：同步 / 备份 / 隐私 / 复习配置 / 云端 OCR / 多档案（N-03/04，F-14/21/22）
import React, { useEffect, useState } from "react";
import {
  Box,
  Button,
  Chip,
  Divider,
  Paper,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from "@mui/material";
import { useAppStore } from "../stores/appStore";
import { useSettingsStore } from "../stores/settingsStore";
import { settingsApi } from "../api/settings";
import { modelApi } from "../api/model";
import { fmtBytes } from "../utils/format";

export default function SystemSettingsPage(): React.JSX.Element {
  const { online, darkMode, toggleDarkMode, notify } = useAppStore();
  const { backups, reviewIntervals, fetchBackups, fetchReviewIntervals, setReviewIntervals } =
    useSettingsStore();

  const [ocrBaseUrl, setOcrBaseUrl] = useState("");
  const [intervalsText, setIntervalsText] = useState("1,3,7,15,30");
  const [restoring, setRestoring] = useState<string | null>(null);

  useEffect(() => {
    void fetchBackups();
    void fetchReviewIntervals();
    settingsApi.get("ocr_cloud_base_url").then((v) => setOcrBaseUrl(v ?? ""));
  }, [fetchBackups, fetchReviewIntervals]);

  useEffect(() => {
    setIntervalsText(reviewIntervals.join(","));
  }, [reviewIntervals]);

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 700 }}>
        系统设置
      </Typography>

      {/* 状态与外观 */}
      <Paper sx={{ p: 2, mb: 2 }}>
        <Typography variant="subtitle2" sx={{ mb: 1 }}>
          状态与外观
        </Typography>
        <Stack direction="row" spacing={2} alignItems="center">
          <Chip color={online ? "success" : "warning"} label={online ? "在线" : "本地模式（离线）"} />
          <Stack direction="row" spacing={1} alignItems="center">
            <Typography variant="body2">暗色模式</Typography>
            <Switch size="small" checked={darkMode} onChange={toggleDarkMode} />
          </Stack>
        </Stack>
      </Paper>

      {/* 云端 OCR */}
      <Paper sx={{ p: 2, mb: 2 }}>
        <Typography variant="subtitle2" sx={{ mb: 1 }}>
          云端公式 OCR（SimpleTex / Mathpix 类）
        </Typography>
        <Stack direction="row" spacing={1}>
          <TextField
            size="small"
            label="API 端点"
            sx={{ flex: 1 }}
            value={ocrBaseUrl}
            onChange={(e) => setOcrBaseUrl(e.target.value)}
          />
          <Button
            size="small"
            variant="contained"
            onClick={async () => {
              await settingsApi.set("ocr_cloud_base_url", ocrBaseUrl.trim());
              notify({ severity: "success", message: "OCR 端点已保存" });
            }}
          >
            保存
          </Button>
          <Button
            size="small"
            onClick={() => {
              const key = window.prompt("输入云端 OCR 的 API Key（写入钥匙串，别名 ocr-cloud）：");
              if (key) {
                void modelApi
                  .keyRotate("ocr-cloud", key)
                  .then(() => notify({ severity: "success", message: "OCR 密钥已保存到钥匙串" }));
              }
            }}
          >
            设置密钥
          </Button>
        </Stack>
        <Typography variant="caption" color="text.secondary">
          未配置时云端 OCR 不可用，系统自动降级本地 Tesseract。
        </Typography>
      </Paper>

      {/* 复习排程 */}
      <Paper sx={{ p: 2, mb: 2 }}>
        <Typography variant="subtitle2" sx={{ mb: 1 }}>
          复习排程（类艾宾浩斯间隔，天）
        </Typography>
        <Stack direction="row" spacing={1}>
          <TextField
            size="small"
            sx={{ flex: 1 }}
            value={intervalsText}
            onChange={(e) => setIntervalsText(e.target.value)}
            helperText="逗号分隔，如 1,3,7,15,30"
          />
          <Button
            size="small"
            variant="contained"
            onClick={async () => {
              const intervals = intervalsText
                .split(/[,，]/)
                .map((s) => Number(s.trim()))
                .filter((n) => Number.isFinite(n) && n > 0);
              if (intervals.length === 0) {
                notify({ severity: "error", message: "间隔格式非法" });
                return;
              }
              await setReviewIntervals(intervals);
              notify({ severity: "success", message: "复习间隔已更新" });
            }}
          >
            保存
          </Button>
        </Stack>
      </Paper>

      {/* 备份 */}
      <Paper sx={{ p: 2, mb: 2 }}>
        <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 1 }}>
          <Typography variant="subtitle2">本地备份（每日自动，保留 7 份）</Typography>
          <Button
            size="small"
            variant="outlined"
            onClick={async () => {
              const path = await settingsApi.backupRun();
              notify({ severity: "success", message: `备份完成：${path}` });
              await fetchBackups();
            }}
          >
            立即备份
          </Button>
        </Stack>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>文件</TableCell>
              <TableCell>大小</TableCell>
              <TableCell>时间</TableCell>
              <TableCell>操作</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {backups.map((b) => (
              <TableRow key={b.path}>
                <TableCell>{b.name}</TableCell>
                <TableCell>{fmtBytes(b.sizeBytes)}</TableCell>
                <TableCell>{b.createdAt}</TableCell>
                <TableCell>
                  <Button
                    size="small"
                    color="warning"
                    disabled={restoring !== null}
                    onClick={async () => {
                      if (!window.confirm(`从 ${b.name} 恢复？当前数据将被覆盖，恢复后请重启应用。`)) return;
                      setRestoring(b.path);
                      try {
                        await settingsApi.backupRestore(b.path);
                        notify({ severity: "success", message: "恢复完成，请重启应用" });
                      } finally {
                        setRestoring(null);
                      }
                    }}
                  >
                    恢复
                  </Button>
                </TableCell>
              </TableRow>
            ))}
            {backups.length === 0 && (
              <TableRow>
                <TableCell colSpan={4} align="center">
                  <Typography variant="body2" color="text.secondary" sx={{ py: 2 }}>
                    暂无备份（应用启动后会自动创建）
                  </Typography>
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </Paper>

      {/* 隐私 */}
      <Paper sx={{ p: 2, mb: 2 }}>
        <Typography variant="subtitle2" sx={{ mb: 1 }}>
          隐私
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
          全部数据仅存本机（SQLCipher 加密）；API Key 存系统钥匙串；日志自动 PII 打码。
        </Typography>
        <Button
          size="small"
          color="error"
          variant="outlined"
          onClick={async () => {
            if (!window.confirm("确认清除全部调用日志与运行日志？此操作不可恢复。")) return;
            const n = await settingsApi.logClear();
            notify({ severity: "success", message: `已清除 ${n} 条调用日志` });
          }}
        >
          一键清除日志
        </Button>
      </Paper>

      {/* 同步与多档案（P1 桩） */}
      <Paper sx={{ p: 2 }}>
        <Typography variant="subtitle2" sx={{ mb: 1 }}>
          云同步与多档案（P1 预留）
        </Typography>
        <Stack direction="row" spacing={1} alignItems="center">
          <Chip size="small" label="默认：仅本地" color="success" />
          <Button
            size="small"
            onClick={async () => {
              try {
                await settingsApi.syncPush();
              } catch {
                /* client 已弹出「未实现」提示 */
              }
            }}
          >
            测试同步推送
          </Button>
        </Stack>
        <Divider sx={{ my: 1 }} />
        <Typography variant="caption" color="text.secondary">
          端到端加密同步（用户自持密钥、服务端仅存密文）与多档案支持将在 P1 版本开放。
        </Typography>
      </Paper>
    </Box>
  );
}
