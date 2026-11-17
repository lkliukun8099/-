// 错题详情：原图 / OCR 编辑 / 归因（确认+修改+重跑）/ 变式生成 / 复习记录 / 标签
import React, { useEffect, useMemo, useState } from "react";
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  Divider,
  Grid,
  Paper,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { useParams } from "react-router-dom";
import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir, join } from "@tauri-apps/api/path";
import { useMistakeStore } from "../stores/mistakeStore";
import { useTrainStore } from "../stores/trainStore";
import { useAppStore } from "../stores/appStore";
import { mistakeApi } from "../api/mistake";
import { ocrApi } from "../api/ocr";
import { attributionApi } from "../api/attribution";
import OcrResultEditor from "../components/OcrResultEditor";
import LatexText from "../components/LatexText";
import TagPicker from "../components/TagPicker";
import {
  difficultyText,
  fmtDateTime,
  originText,
  statusColor,
  statusText,
  validateStatusText,
  variantTypeText,
} from "../utils/format";

export default function MistakeDetailPage(): React.JSX.Element {
  const { id } = useParams<{ id: string }>();
  const mistakeId = Number(id);
  const notify = useAppStore((s) => s.notify);
  const online = useAppStore((s) => s.online);
  const { detail, detailLoading, tags, fetchDetail, fetchTags } = useMistakeStore();
  const { drafts, generating, generate, clearDrafts } = useTrainStore();

  const [imageUrl, setImageUrl] = useState<string>("");
  const [studentAnswer, setStudentAnswer] = useState("");
  const [correctAnswer, setCorrectAnswer] = useState("");
  const [running, setRunning] = useState<string | null>(null);

  useEffect(() => {
    void fetchDetail(mistakeId);
    void fetchTags();
    clearDrafts();
    return () => clearDrafts();
  }, [mistakeId, fetchDetail, fetchTags, clearDrafts]);

  const mistake = detail?.mistake;

  useEffect(() => {
    setStudentAnswer(mistake?.studentAnswer ?? "");
    setCorrectAnswer(mistake?.correctAnswer ?? "");
  }, [mistake?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  // 图片 URL（asset 协议）
  useEffect(() => {
    (async () => {
      if (!mistake) return;
      const rel = mistake.processedImage ?? mistake.sourceImage;
      const dir = await appDataDir();
      setImageUrl(convertFileSrc(await join(dir, rel)));
    })();
  }, [mistake]);

  const activeAttr = useMemo(
    () => detail?.attributions.find((a) => a.isActive) ?? null,
    [detail]
  );

  if (detailLoading || !mistake || !detail) {
    return (
      <Box sx={{ display: "flex", justifyContent: "center", mt: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  const saveOcr = async (text: string, latex: string) => {
    await mistakeApi.update(mistakeId, {
      ocrText: text,
      ocrLatex: latex || null,
      version: mistake.version,
    });
    notify({ severity: "success", message: "OCR 结果已保存" });
    await fetchDetail(mistakeId);
  };

  const saveAnswers = async () => {
    await mistakeApi.update(mistakeId, {
      studentAnswer,
      correctAnswer,
      version: mistake.version,
    });
    notify({ severity: "success", message: "作答信息已保存" });
    await fetchDetail(mistakeId);
  };

  const runOcr = async (prefer: "auto" | "cloud" | "offline") => {
    setRunning("ocr");
    try {
      await ocrApi.run(mistakeId, prefer);
      notify({ severity: "success", message: "OCR 完成" });
      await fetchDetail(mistakeId);
    } catch {
      /* client 已弹错 */
    } finally {
      setRunning(null);
    }
  };

  const runAttribution = async (mode: "local" | "cloud") => {
    setRunning("attr");
    try {
      await attributionApi.run(mistakeId, mode);
      notify({ severity: "success", message: mode === "cloud" ? "云端深判完成" : "本地初判完成" });
      await fetchDetail(mistakeId);
    } catch {
      /* client 已弹错 */
    } finally {
      setRunning(null);
    }
  };

  const confirmAttr = async (edited?: { label: string; detail: string }) => {
    if (!activeAttr) return;
    await attributionApi.confirm({
      attributionId: activeAttr.id,
      editedLabel: edited?.label,
      editedDetail: edited?.detail,
    });
    notify({ severity: "success", message: edited ? "归因已修改" : "归因已确认" });
    await fetchDetail(mistakeId);
  };

  return (
    <Box>
      <Stack direction="row" spacing={1} alignItems="center" sx={{ mb: 2 }}>
        <Typography variant="h5" fontWeight={700}>
          错题 #{mistake.id}
        </Typography>
        <Chip size="small" color={statusColor(mistake.status)} label={statusText(mistake.status)} />
        <Chip size="small" variant="outlined" label={mistake.subjectName ?? "未分学科"} />
        <Typography variant="caption" color="warning.main">
          {difficultyText(mistake.difficulty)}
        </Typography>
      </Stack>

      <Grid container spacing={2}>
        {/* 左栏：图片 + OCR */}
        <Grid item xs={12} md={6}>
          <Paper sx={{ p: 2, mb: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              原图
            </Typography>
            {imageUrl && (
              <img
                src={imageUrl}
                alt="错题原图"
                style={{ maxWidth: "100%", borderRadius: 8, border: "1px solid #ddd" }}
              />
            )}
          </Paper>
          <Paper sx={{ p: 2 }}>
            <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 1 }}>
              <Typography variant="subtitle2">OCR 识别结果</Typography>
              <Stack direction="row" spacing={1}>
                <Button size="small" disabled={running !== null} onClick={() => runOcr("auto")}>
                  重跑 OCR
                </Button>
                <Button size="small" disabled={running !== null || !online} onClick={() => runOcr("cloud")}>
                  云端识别
                </Button>
              </Stack>
            </Stack>
            {mistake.ocrEngine && (
              <Typography variant="caption" color="text.secondary">
                引擎：{mistake.ocrEngine} · 置信度：
                {mistake.ocrConfidence ? (mistake.ocrConfidence * 100).toFixed(0) + "%" : "-"}
              </Typography>
            )}
            <OcrResultEditor
              initialText={mistake.ocrText ?? ""}
              initialLatex={mistake.ocrLatex ?? ""}
              onSave={saveOcr}
            />
          </Paper>
        </Grid>

        {/* 右栏：作答 + 归因 + 变式 */}
        <Grid item xs={12} md={6}>
          <Paper sx={{ p: 2, mb: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              作答信息
            </Typography>
            <TextField
              size="small"
              fullWidth
              label="学生作答"
              value={studentAnswer}
              onChange={(e) => setStudentAnswer(e.target.value)}
            />
            <TextField
              size="small"
              fullWidth
              label="正确答案"
              sx={{ mt: 1 }}
              value={correctAnswer}
              onChange={(e) => setCorrectAnswer(e.target.value)}
            />
            <Button size="small" variant="outlined" sx={{ mt: 1 }} onClick={saveAnswers}>
              保存作答
            </Button>
          </Paper>

          <Paper sx={{ p: 2, mb: 2 }}>
            <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 1 }}>
              <Typography variant="subtitle2">错因归因</Typography>
              <Stack direction="row" spacing={1}>
                <Button size="small" disabled={running !== null} onClick={() => runAttribution("local")}>
                  本地初判
                </Button>
                <Button size="small" disabled={running !== null || !online} onClick={() => runAttribution("cloud")}>
                  云端深判
                </Button>
              </Stack>
            </Stack>
            {activeAttr ? (
              <Box>
                <Stack direction="row" spacing={1} alignItems="center">
                  <Chip
                    size="small"
                    color={activeAttr.origin === "cloud" ? "primary" : activeAttr.origin === "user" ? "success" : "default"}
                    label={originText(activeAttr.origin)}
                  />
                  <Typography variant="subtitle2">{activeAttr.causeLabel}</Typography>
                  <Typography variant="caption" color="text.secondary">
                    置信度 {(activeAttr.confidence * 100).toFixed(0)}%
                  </Typography>
                </Stack>
                <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
                  {activeAttr.causeDetail}
                </Typography>
                {activeAttr.kgPath.length > 0 && (
                  <Typography variant="caption" display="block" sx={{ mt: 0.5 }}>
                    知识点：{activeAttr.kgPath.join(" → ")}
                  </Typography>
                )}
                <Stack direction="row" spacing={1} sx={{ mt: 1 }}>
                  <Button size="small" variant="contained" onClick={() => confirmAttr()}>
                    认可
                  </Button>
                  <Button
                    size="small"
                    onClick={() => {
                      const label = window.prompt("修改为错因标签：", activeAttr.causeLabel);
                      if (label) void confirmAttr({ label, detail: activeAttr.causeDetail ?? "" });
                    }}
                  >
                    修改
                  </Button>
                </Stack>
              </Box>
            ) : (
              <Typography variant="body2" color="text.secondary">
                尚未归因，点击上方按钮运行分析。
              </Typography>
            )}
            {detail.attributions.length > 1 && (
              <>
                <Divider sx={{ my: 1 }} />
                <Typography variant="caption" color="text.secondary">
                  历史版本（{detail.attributions.length - 1} 条，双版本永久保留）
                </Typography>
              </>
            )}
          </Paper>

          <Paper sx={{ p: 2 }}>
            <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 1 }}>
              <Typography variant="subtitle2">变式训练</Typography>
              <Button
                size="small"
                variant="contained"
                disabled={generating || !online}
                onClick={() => generate(mistakeId, 2)}
              >
                {generating ? "生成与校验中…" : "生成变式题 ×2"}
              </Button>
            </Stack>
            {!online && (
              <Typography variant="caption" color="warning.main">
                离线模式：变式生成需要联网
              </Typography>
            )}
            {/* 流式生成中的草稿 */}
            {drafts.map((d, i) => (
              <Box key={`draft-${i}`} sx={{ p: 1, mb: 1, bgcolor: "action.hover", borderRadius: 1 }}>
                <Stack direction="row" spacing={1} alignItems="center">
                  <Chip size="small" label={variantTypeText(d.variantType)} />
                  <Chip
                    size="small"
                    color={d.validateStatus === "passed" ? "success" : "warning"}
                    label={validateStatusText(d.validateStatus)}
                  />
                </Stack>
                <LatexText text={d.content} />
              </Box>
            ))}
            {/* 已入库变式 */}
            {detail.variants.map((v) => (
              <Box key={v.id} sx={{ p: 1, mb: 1, border: 1, borderColor: "divider", borderRadius: 1 }}>
                <Stack direction="row" spacing={1} alignItems="center">
                  <Chip size="small" label={variantTypeText(v.variantType)} />
                  <Chip
                    size="small"
                    color={v.validateStatus === "passed" ? "success" : "warning"}
                    label={validateStatusText(v.validateStatus)}
                  />
                  <Typography variant="caption" color="warning.main">
                    {difficultyText(v.difficulty)}
                  </Typography>
                </Stack>
                <LatexText text={v.content} />
              </Box>
            ))}
          </Paper>
        </Grid>

        {/* 底部：标签 + 复习记录 */}
        <Grid item xs={12}>
          <Paper sx={{ p: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              标签
            </Typography>
            <TagPicker
              allTags={tags}
              selectedIds={detail.tags.map((t) => t.id)}
              onChange={async (ids) => {
                await mistakeApi.setTags(mistakeId, ids);
                await fetchDetail(mistakeId);
              }}
              onCreate={async (name) => {
                const id = await mistakeApi.tagCreate(name);
                await fetchTags();
                return id;
              }}
            />
            {detail.reviewPlans.length > 0 && (
              <>
                <Typography variant="subtitle2" sx={{ mt: 2, mb: 0.5 }}>
                  复习记录
                </Typography>
                {detail.reviewPlans.map((p) => (
                  <Typography key={p.id} variant="caption" display="block">
                    阶段 {p.stage} · 下次复习 {fmtDateTime(p.nextDueAt)} · {p.status}
                  </Typography>
                ))}
              </>
            )}
          </Paper>
        </Grid>
      </Grid>
    </Box>
  );
}
