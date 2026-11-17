// 路由策略表单（F-19）：主模型 + 备用链 + 超时 + 预算
import React, { useState } from "react";
import { Box, Button, Chip, Stack, TextField, Typography } from "@mui/material";
import type { RoutePolicy } from "../types";

interface ModelRouteFormProps {
  policy: RoutePolicy;
  saving?: boolean;
  onSave: (policy: RoutePolicy) => void | Promise<void>;
}

export default function ModelRouteForm({
  policy,
  saving = false,
  onSave,
}: ModelRouteFormProps): React.JSX.Element {
  const [primary, setPrimary] = useState(policy.primary);
  const [fallbacksText, setFallbacksText] = useState(policy.fallbacks.join(", "));
  const [timeoutMs, setTimeoutMs] = useState(policy.timeoutMs);
  const [maxCost, setMaxCost] = useState(policy.maxCostCny?.toString() ?? "");
  const [budget, setBudget] = useState(policy.monthlyBudgetCny?.toString() ?? "");

  const handleSave = async () => {
    const fallbacks = fallbacksText
      .split(/[,，]/)
      .map((s) => s.trim())
      .filter(Boolean);
    await onSave({
      ...policy,
      primary: primary.trim(),
      fallbacks,
      timeoutMs: Math.max(1000, timeoutMs),
      maxCostCny: maxCost ? Number(maxCost) : null,
      monthlyBudgetCny: budget ? Number(budget) : null,
    });
  };

  return (
    <Box sx={{ p: 2, border: 1, borderColor: "divider", borderRadius: 2 }}>
      <Stack direction="row" spacing={1} alignItems="center" sx={{ mb: 1.5 }}>
        <Chip size="small" color="primary" label={policy.taskType} />
        <Typography variant="caption" color="text.secondary">
          格式：provider.model
        </Typography>
      </Stack>
      <Stack spacing={1.5}>
        <TextField
          size="small"
          label="主模型"
          value={primary}
          onChange={(e) => setPrimary(e.target.value)}
          helperText="如 deepseek.deepseek-chat"
        />
        <TextField
          size="small"
          label="备用链（逗号分隔，按序降级）"
          value={fallbacksText}
          onChange={(e) => setFallbacksText(e.target.value)}
          helperText="如 glm.glm-4-flash, kimi.moonshot-v1-8k"
        />
        <Stack direction="row" spacing={1.5}>
          <TextField
            size="small"
            type="number"
            label="超时 (ms)"
            value={timeoutMs}
            onChange={(e) => setTimeoutMs(Number(e.target.value))}
          />
          <TextField
            size="small"
            type="number"
            label="单次成本上限（分）"
            value={maxCost}
            onChange={(e) => setMaxCost(e.target.value)}
          />
          <TextField
            size="small"
            type="number"
            label="月度预算（分）"
            value={budget}
            onChange={(e) => setBudget(e.target.value)}
          />
        </Stack>
        <Box>
          <Button size="small" variant="contained" disabled={saving} onClick={handleSave}>
            保存策略
          </Button>
        </Box>
      </Stack>
    </Box>
  );
}
