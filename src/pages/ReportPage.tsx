// 诊断报告：知识薄弱热力图 + 掌握度曲线 + 错因分布 + 复习日历（F-13）
import React, { useEffect, useState } from "react";
import {
  Box,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Grid,
  MenuItem,
  Paper,
  Select,
  Stack,
  Typography,
} from "@mui/material";
import { useReportStore } from "../stores/reportStore";
import { reviewApi } from "../api/review";
import KgHeatmap from "../components/KgHeatmap";
import MasteryChart from "../components/MasteryChart";
import ReviewCalendar from "../components/ReviewCalendar";
import type { ReviewPlan } from "../types";

export default function ReportPage(): React.JSX.Element {
  const { report, rangeDays, loading, setRangeDays, fetchReport } = useReportStore();
  const [upcoming, setUpcoming] = useState<ReviewPlan[]>([]);

  useEffect(() => {
    void fetchReport();
    reviewApi
      .upcoming(53)
      .then(setUpcoming)
      .catch(() => setUpcoming([]));
  }, [fetchReport]);

  if (!report && loading) {
    return (
      <Box sx={{ display: "flex", justifyContent: "center", mt: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Box>
      <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 2 }}>
        <Typography variant="h5" fontWeight={700}>
          诊断报告
        </Typography>
        <Select
          size="small"
          value={rangeDays}
          onChange={(e) => setRangeDays(Number(e.target.value))}
        >
          <MenuItem value={7}>近 7 天</MenuItem>
          <MenuItem value={30}>近 30 天</MenuItem>
          <MenuItem value={90}>近 90 天</MenuItem>
        </Select>
      </Stack>

      {/* 概览卡片 */}
      <Grid container spacing={2} sx={{ mb: 2 }}>
        <Grid item xs={6} md={3}>
          <Card>
            <CardContent>
              <Typography variant="h4" color="primary">
                {report?.totalMistakes ?? 0}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                错题总数
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={6} md={3}>
          <Card>
            <CardContent>
              <Typography variant="h4" color="success.main">
                {report?.reviewedToday ?? 0}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                今日已复习
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={6} md={3}>
          <Card>
            <CardContent>
              <Typography variant="h4" color="warning.main">
                {report?.heatmap.filter((c) => c.score < 0.5).length ?? 0}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                薄弱知识点（&lt;50%）
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={6} md={3}>
          <Card>
            <CardContent>
              <Typography variant="h4">
                {report?.curve.length
                  ? `${(report.curve[report.curve.length - 1].avgScore * 100).toFixed(0)}%`
                  : "-"}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                最新平均掌握度
              </Typography>
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      <Grid container spacing={2}>
        <Grid item xs={12} md={7}>
          <Paper sx={{ p: 2, mb: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              知识薄弱热力图
            </Typography>
            {report && report.heatmap.length > 0 ? (
              <KgHeatmap cells={report.heatmap} />
            ) : (
              <Typography variant="body2" color="text.secondary">
                暂无作答数据
              </Typography>
            )}
          </Paper>
        </Grid>
        <Grid item xs={12} md={5}>
          <Paper sx={{ p: 2, mb: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              掌握度曲线
            </Typography>
            {report && report.curve.length > 0 ? (
              <MasteryChart points={report.curve} />
            ) : (
              <Typography variant="body2" color="text.secondary">
                暂无历史数据
              </Typography>
            )}
          </Paper>
        </Grid>
        <Grid item xs={12} md={7}>
          <Paper sx={{ p: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              复习计划日历
            </Typography>
            <ReviewCalendar plans={upcoming} />
          </Paper>
        </Grid>
        <Grid item xs={12} md={5}>
          <Paper sx={{ p: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              错因分布（Top 20）
            </Typography>
            {report && report.causeDistribution.length > 0 ? (
              <Stack spacing={0.5}>
                {report.causeDistribution.map((c) => (
                  <Stack key={c.causeLabel} direction="row" spacing={1} alignItems="center">
                    <Chip size="small" label={c.count} sx={{ width: 44 }} />
                    <Typography variant="body2">{c.causeLabel}</Typography>
                  </Stack>
                ))}
              </Stack>
            ) : (
              <Typography variant="body2" color="text.secondary">
                暂无归因数据
              </Typography>
            )}
          </Paper>
        </Grid>
      </Grid>
    </Box>
  );
}
