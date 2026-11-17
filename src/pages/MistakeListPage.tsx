// 错题本列表：多维筛选 + FTS 搜索 + 分页 + 标签（F-05）
import React, { useEffect } from "react";
import {
  Box,
  Chip,
  FormControl,
  IconButton,
  InputLabel,
  MenuItem,
  Pagination,
  Paper,
  Select,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from "@mui/material";
import DeleteIcon from "@mui/icons-material/Delete";
import { useNavigate } from "react-router-dom";
import { useMistakeStore } from "../stores/mistakeStore";
import { SUBJECTS } from "../types";
import {
  difficultyText,
  fmtDateTime,
  statusColor,
  statusText,
} from "../utils/format";

const STATUS_OPTIONS = [
  { value: "pending_analysis", label: "待深度分析" },
  { value: "analyzing", label: "分析中" },
  { value: "analyzed", label: "已分析" },
  { value: "conflict", label: "冲突待合并" },
  { value: "archived", label: "已归档" },
];

export default function MistakeListPage(): React.JSX.Element {
  const navigate = useNavigate();
  const { query, pageData, loading, tags, setQuery, fetchList, fetchTags, remove } =
    useMistakeStore();

  useEffect(() => {
    void fetchList();
    void fetchTags();
  }, [fetchList, fetchTags]);

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 700 }}>
        错题本
      </Typography>

      {/* 筛选栏 */}
      <Paper sx={{ p: 1.5, mb: 2 }}>
        <Stack direction="row" spacing={1.5} flexWrap="wrap" useFlexGap>
          <TextField
            size="small"
            label="全文搜索"
            sx={{ minWidth: 200 }}
            value={query.keyword ?? ""}
            onChange={(e) => setQuery({ keyword: e.target.value || null })}
          />
          <FormControl size="small" sx={{ minWidth: 120 }}>
            <InputLabel>学科</InputLabel>
            <Select
              label="学科"
              value={query.subjectId ?? ""}
              onChange={(e) =>
                setQuery({ subjectId: e.target.value === "" ? null : Number(e.target.value) })
              }
            >
              <MenuItem value="">全部</MenuItem>
              {SUBJECTS.map((s) => (
                <MenuItem key={s.id} value={s.id}>
                  {s.name}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
          <FormControl size="small" sx={{ minWidth: 140 }}>
            <InputLabel>状态</InputLabel>
            <Select
              label="状态"
              value={query.status ?? ""}
              onChange={(e) => setQuery({ status: e.target.value === "" ? null : String(e.target.value) })}
            >
              <MenuItem value="">全部</MenuItem>
              {STATUS_OPTIONS.map((s) => (
                <MenuItem key={s.value} value={s.value}>
                  {s.label}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
          <FormControl size="small" sx={{ minWidth: 120 }}>
            <InputLabel>标签</InputLabel>
            <Select
              label="标签"
              value={query.tagId ?? ""}
              onChange={(e) =>
                setQuery({ tagId: e.target.value === "" ? null : Number(e.target.value) })
              }
            >
              <MenuItem value="">全部</MenuItem>
              {tags.map((t) => (
                <MenuItem key={t.id} value={t.id}>
                  {t.name}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
          <TextField
            size="small"
            label="起始日期"
            type="date"
            InputLabelProps={{ shrink: true }}
            value={query.dateFrom ?? ""}
            onChange={(e) => setQuery({ dateFrom: e.target.value || null })}
          />
          <TextField
            size="small"
            label="结束日期"
            type="date"
            InputLabelProps={{ shrink: true }}
            value={query.dateTo ?? ""}
            onChange={(e) => setQuery({ dateTo: e.target.value || null })}
          />
        </Stack>
      </Paper>

      {/* 列表 */}
      <Paper>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>题干摘要</TableCell>
              <TableCell width={80}>学科</TableCell>
              <TableCell width={110}>状态</TableCell>
              <TableCell width={70}>难度</TableCell>
              <TableCell width={160}>标签</TableCell>
              <TableCell width={120}>录入时间</TableCell>
              <TableCell width={60}>操作</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {(pageData?.items ?? []).map((m) => (
              <TableRow
                key={m.id}
                hover
                sx={{ cursor: "pointer" }}
                onClick={() => navigate(`/mistakes/${m.id}`)}
              >
                <TableCell>
                  <Typography
                    variant="body2"
                    sx={{
                      maxWidth: 420,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {m.ocrText ?? "（识别中…）"}
                  </Typography>
                </TableCell>
                <TableCell>{m.subjectName ?? "-"}</TableCell>
                <TableCell>
                  <Chip size="small" color={statusColor(m.status)} label={statusText(m.status)} />
                </TableCell>
                <TableCell>
                  <Typography variant="caption" color="warning.main">
                    {difficultyText(m.difficulty)}
                  </Typography>
                </TableCell>
                <TableCell>
                  {m.tags.slice(0, 2).map((t) => (
                    <Chip key={t} size="small" variant="outlined" label={t} sx={{ mr: 0.5 }} />
                  ))}
                </TableCell>
                <TableCell>{fmtDateTime(m.createdAt)}</TableCell>
                <TableCell>
                  <IconButton
                    size="small"
                    onClick={(e) => {
                      e.stopPropagation();
                      if (window.confirm(`确认删除错题 #${m.id}？`)) {
                        void remove(m.id);
                      }
                    }}
                  >
                    <DeleteIcon fontSize="small" />
                  </IconButton>
                </TableCell>
              </TableRow>
            ))}
            {pageData && pageData.items.length === 0 && (
              <TableRow>
                <TableCell colSpan={7} align="center">
                  <Typography variant="body2" color="text.secondary" sx={{ py: 4 }}>
                    暂无错题，去「拍照录入」添加第一道吧
                  </Typography>
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
        {pageData && pageData.total > 0 && (
          <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ p: 1.5 }}>
            <Typography variant="caption" color="text.secondary">
              共 {pageData.total} 条{loading ? " · 加载中…" : ""}
            </Typography>
            <Pagination
              size="small"
              page={pageData.page}
              count={Math.max(1, Math.ceil(pageData.total / pageData.pageSize))}
              onChange={(_, p) => setQuery({ page: p })}
            />
          </Stack>
        )}
      </Paper>
    </Box>
  );
}
