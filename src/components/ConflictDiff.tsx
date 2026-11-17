// 冲突双版本对比组件（F-16/AC-8）：本地初判 vs 云端深判，支持三方裁决
import React, { useState } from "react";
import {
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import type { Attribution, Conflict } from "../types";
import { originText } from "../utils/format";

interface ConflictDiffProps {
  conflict: Conflict;
  resolving: boolean;
  onResolve: (
    conflictId: number,
    choice: "local" | "cloud" | "merged",
    mergedLabel?: string,
    mergedDetail?: string
  ) => void | Promise<void>;
}

/** 单侧归因卡片 */
function AttrCard({
  attr,
  highlight,
}: {
  attr: Attribution;
  highlight: boolean;
}): React.JSX.Element {
  return (
    <Box
      sx={{
        flex: 1,
        p: 1.5,
        border: 1,
        borderColor: highlight ? "primary.main" : "divider",
        borderRadius: 1,
      }}
    >
      <Chip
        size="small"
        label={originText(attr.origin)}
        color={attr.origin === "local" ? "default" : "primary"}
        sx={{ mb: 1 }}
      />
      <Typography variant="subtitle2">{attr.causeLabel}</Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
        {attr.causeDetail ?? "（无说明）"}
      </Typography>
      <Typography variant="caption" display="block" sx={{ mt: 0.5 }}>
        知识点：{attr.kgPath.length > 0 ? attr.kgPath.join(" → ") : "未定位"}
      </Typography>
      <Typography variant="caption" color="text.secondary">
        置信度：{(attr.confidence * 100).toFixed(0)}%
      </Typography>
    </Box>
  );
}

export default function ConflictDiff({
  conflict,
  resolving,
  onResolve,
}: ConflictDiffProps): React.JSX.Element {
  const [mergeOpen, setMergeOpen] = useState(false);
  const [mergedLabel, setMergedLabel] = useState("");
  const [mergedDetail, setMergedDetail] = useState("");

  const sameLabel = conflict.localAttr.causeLabel === conflict.cloudAttr.causeLabel;

  return (
    <Box sx={{ p: 2, border: 1, borderColor: "divider", borderRadius: 2 }}>
      <Typography variant="subtitle2" sx={{ mb: 1 }}>
        错题 #{conflict.entityId} · {sameLabel ? "标签一致但知识点不同" : "双版本归因不一致"}
      </Typography>
      <Stack direction={{ xs: "column", md: "row" }} spacing={1.5}>
        <AttrCard attr={conflict.localAttr} highlight={false} />
        <Divider orientation="vertical" flexItem sx={{ display: { xs: "none", md: "block" } }} />
        <AttrCard attr={conflict.cloudAttr} highlight />
      </Stack>
      <Stack direction="row" spacing={1} sx={{ mt: 1.5 }}>
        <Button
          size="small"
          variant="outlined"
          disabled={resolving}
          onClick={() => onResolve(conflict.id, "local")}
        >
          采用本地初判
        </Button>
        <Button
          size="small"
          variant="contained"
          disabled={resolving}
          onClick={() => onResolve(conflict.id, "cloud")}
        >
          采用云端深判
        </Button>
        <Button size="small" disabled={resolving} onClick={() => setMergeOpen(true)}>
          人工合并
        </Button>
      </Stack>

      {/* 人工合并对话框 */}
      <Dialog open={mergeOpen} onClose={() => setMergeOpen(false)} maxWidth="sm" fullWidth>
        <DialogTitle>人工合并归因</DialogTitle>
        <DialogContent>
          <TextField
            label="合并后错因标签"
            fullWidth
            required
            size="small"
            sx={{ mt: 1 }}
            value={mergedLabel}
            onChange={(e) => setMergedLabel(e.target.value)}
          />
          <TextField
            label="合并后说明"
            fullWidth
            multiline
            minRows={3}
            size="small"
            sx={{ mt: 1.5 }}
            value={mergedDetail}
            onChange={(e) => setMergedDetail(e.target.value)}
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setMergeOpen(false)}>取消</Button>
          <Button
            variant="contained"
            disabled={!mergedLabel.trim() || resolving}
            onClick={async () => {
              await onResolve(conflict.id, "merged", mergedLabel.trim(), mergedDetail.trim());
              setMergeOpen(false);
            }}
          >
            确认合并
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
