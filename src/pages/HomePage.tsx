// 今日面板：今日复习任务 / 待分析队列 / 薄弱预警（首页）
import React, { useEffect, useState } from "react";
import {
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Grid,
  Stack,
  Typography,
} from "@mui/material";
import { useNavigate } from "react-router-dom";
import { reviewApi } from "../api/review";
import { mistakeApi } from "../api/mistake";
import { attributionApi } from "../api/attribution";
import { masteryApi } from "../api/mastery";
import { useAppStore } from "../stores/appStore";
import type { HeatCell, ReviewItem } from "../types";
import { fmtDateTime } from "../utils/format";

export default function HomePage(): React.JSX.Element {
  const navigate = useNavigate();
  const online = useAppStore((s) => s.online);
  const [loading, setLoading] = useState(true);
  const [todayReviews, setTodayReviews] = useState<ReviewItem[]>([]);
  const [pendingCount, setPendingCount] = useState(0);
  const [conflictCount, setConflictCount] = useState(0);
  const [weakCells, setWeakCells] = useState<HeatCell[]>([]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setLoading(true);
      try {
        const [reviews, pendingPage, conflicts, report] = await Promise.all([
          reviewApi.today().catch(() => [] as ReviewItem[]),
          mistakeApi
            .list({ status: "pending_analysis", page: 1, pageSize: 1 })
            .catch(() => ({ total: 0 })),
          attributionApi.conflictList().catch(() => []),
          masteryApi.report(30).catch(() => null),
        ]);
        if (cancelled) return;
        setTodayReviews(reviews);
        setPendingCount(pendingPage.total);
        setConflictCount(conflicts.length);
        setWeakCells(report?.heatmap.slice(0, 5) ?? []);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  if (loading) {
    return (
      <Box sx={{ display: "flex", justifyContent: "center", mt: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 700 }}>
        今日面板
      </Typography>
      <Grid container spacing={2}>
        {/* 今日复习 */}
        <Grid item xs={12} md={4}>
          <Card>
            <CardContent>
              <Stack direction="row" justifyContent="space-between" alignItems="center">
                <Typography variant="subtitle1" fontWeight={600}>
                  今日复习
                </Typography>
                <Chip color="primary" size="small" label={`${todayReviews.length} 项`} />
              </Stack>
              {todayReviews.length === 0 ? (
                <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                  今天没有到期复习，去录入新错题吧。
                </Typography>
              ) : (
                todayReviews.slice(0, 4).map((item) => (
                  <Typography
                    key={item.plan.id}
                    variant="body2"
                    sx={{
                      mt: 1,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    · {item.title}
                  </Typography>
                ))
              )}
              <Button size="small" sx={{ mt: 1.5 }} variant="contained" onClick={() => navigate("/train")}>
                开始训练
              </Button>
            </CardContent>
          </Card>
        </Grid>

        {/* 待分析队列 */}
        <Grid item xs={12} md={4}>
          <Card>
            <CardContent>
              <Stack direction="row" justifyContent="space-between" alignItems="center">
                <Typography variant="subtitle1" fontWeight={600}>
                  待深度分析
                </Typography>
                <Chip
                  size="small"
                  color={pendingCount > 0 ? "warning" : "default"}
                  label={`${pendingCount} 题`}
                />
              </Stack>
              <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                {online
                  ? pendingCount > 0
                    ? "可进入错题详情触发云端深判，或在错题本中批量查看。"
                    : "所有错题均已完成分析。"
                  : "当前离线：新录入错题将先做本地规则初判，联网后自动补齐深度分析。"}
              </Typography>
              <Button size="small" sx={{ mt: 1.5 }} onClick={() => navigate("/mistakes")}>
                查看错题本
              </Button>
            </CardContent>
          </Card>
        </Grid>

        {/* 冲突合并 */}
        <Grid item xs={12} md={4}>
          <Card>
            <CardContent>
              <Stack direction="row" justifyContent="space-between" alignItems="center">
                <Typography variant="subtitle1" fontWeight={600}>
                  冲突待合并
                </Typography>
                <Chip
                  size="small"
                  color={conflictCount > 0 ? "error" : "default"}
                  label={`${conflictCount} 条`}
                />
              </Stack>
              <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                本地初判与云端深判不一致时需要你裁决，双版本永久保留。
              </Typography>
              <Button
                size="small"
                sx={{ mt: 1.5 }}
                color="error"
                disabled={conflictCount === 0}
                onClick={() => navigate("/conflicts")}
              >
                去合并
              </Button>
            </CardContent>
          </Card>
        </Grid>

        {/* 薄弱预警 */}
        <Grid item xs={12}>
          <Card>
            <CardContent>
              <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 1 }}>
                薄弱知识点预警（Top 5）
              </Typography>
              {weakCells.length === 0 ? (
                <Typography variant="body2" color="text.secondary">
                  暂无作答数据，完成几次变式训练后这里会给出薄弱预警。
                </Typography>
              ) : (
                <Stack spacing={0.5}>
                  {weakCells.map((cell) => (
                    <Stack key={cell.kgNodeId} direction="row" spacing={1} alignItems="center">
                      <Chip
                        size="small"
                        color={cell.score < 0.4 ? "error" : "warning"}
                        label={`${(cell.score * 100).toFixed(0)}%`}
                        sx={{ width: 64 }}
                      />
                      <Typography variant="body2">
                        {cell.nodePath.join(" → ") || cell.nodeName}（作答 {cell.attempts} 次）
                      </Typography>
                    </Stack>
                  ))}
                  <Button size="small" sx={{ alignSelf: "flex-start", mt: 1 }} onClick={() => navigate("/report")}>
                    查看完整诊断报告
                  </Button>
                </Stack>
              )}
            </CardContent>
          </Card>
        </Grid>
      </Grid>
      <Typography variant="caption" color="text.secondary" sx={{ mt: 2, display: "block" }}>
        数据更新于 {fmtDateTime(new Date().toISOString())} · 当前 {online ? "在线" : "本地模式（离线）"}
      </Typography>
    </Box>
  );
}
