// 模型与 AI 设置：供应商管理（密钥走钥匙串）/ 路由策略 / 成本面板（F-10/11/19）
import React, { useEffect, useState } from "react";
import {
  Box,
  Button,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControl,
  IconButton,
  InputLabel,
  MenuItem,
  Paper,
  Select,
  Stack,
  Switch,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tabs,
  TextField,
  Typography,
} from "@mui/material";
import DeleteIcon from "@mui/icons-material/Delete";
import KeyIcon from "@mui/icons-material/Key";
import NetworkCheckIcon from "@mui/icons-material/NetworkCheck";
import { useSettingsStore } from "../stores/settingsStore";
import { useAppStore } from "../stores/appStore";
import { modelApi } from "../api/model";
import ModelRouteForm from "../components/ModelRouteForm";
import type { ModelProvider, ModelSpec, ProviderInput } from "../types";
import { fmtCost, fmtDateTime } from "../utils/format";

/** 解析模型清单文本：每行 "id, 输入价, 输出价, 上下文"（分/千token） */
function parseModels(text: string): ModelSpec[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [id, priceIn, priceOut, ctx] = line.split(",").map((s) => s.trim());
      return {
        id: id ?? "",
        priceIn: Number(priceIn) || 0,
        priceOut: Number(priceOut) || 0,
        ctx: Number(ctx) || 4096,
      };
    })
    .filter((m) => m.id);
}

function modelsToText(models: ModelSpec[]): string {
  return models.map((m) => `${m.id}, ${m.priceIn}, ${m.priceOut}, ${m.ctx}`).join("\n");
}

/** 供应商编辑对话框 */
function ProviderDialog({
  open,
  provider,
  onClose,
  onSave,
}: {
  open: boolean;
  provider: ModelProvider | null;
  onClose: () => void;
  onSave: (input: ProviderInput) => Promise<void>;
}): React.JSX.Element {
  const [name, setName] = useState("");
  const [protocol, setProtocol] = useState("openai_compat");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [modelsText, setModelsText] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      setName(provider?.name ?? "");
      setProtocol(provider?.protocol ?? "openai_compat");
      setBaseUrl(provider?.baseUrl ?? "");
      setApiKey(""); // 密钥永不回显（隐私要求 7）
      setModelsText(provider ? modelsToText(provider.models) : "");
    }
  }, [open, provider]);

  const handleSave = async () => {
    setSaving(true);
    try {
      await onSave({
        id: provider?.id,
        name,
        protocol,
        baseUrl,
        apiKey: apiKey || undefined,
        models: parseModels(modelsText),
        enabled: provider?.enabled ?? true,
        priority: provider?.priority ?? 0,
      });
      onClose();
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onClose={onClose} maxWidth="sm" fullWidth>
      <DialogTitle>{provider ? "编辑供应商" : "新增供应商"}</DialogTitle>
      <DialogContent>
        <Stack spacing={1.5} sx={{ mt: 1 }}>
          <TextField size="small" label="名称（如 deepseek/glm/kimi/hunyuan/local）" value={name} onChange={(e) => setName(e.target.value)} />
          <FormControl size="small">
            <InputLabel>协议</InputLabel>
            <Select label="协议" value={protocol} onChange={(e) => setProtocol(e.target.value)}>
              <MenuItem value="openai_compat">OpenAI 兼容（DeepSeek/GLM/Kimi/本地）</MenuItem>
              <MenuItem value="hunyuan_tc3">腾讯混元 TC3 签名</MenuItem>
            </Select>
          </FormControl>
          <TextField
            size="small"
            label="Base URL（不含 /chat/completions）"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://api.deepseek.com/v1"
          />
          <TextField
            size="small"
            type="password"
            label={provider ? "API Key（留空表示不更换）" : "API Key"}
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            helperText="密钥加密存入系统钥匙串，永不回传"
          />
          <TextField
            size="small"
            label="模型清单（每行：模型ID, 输入价, 输出价, 上下文）"
            multiline
            minRows={3}
            value={modelsText}
            onChange={(e) => setModelsText(e.target.value)}
            placeholder={"deepseek-chat, 1, 2, 64000\ndeepseek-reasoner, 4, 16, 64000"}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button variant="contained" disabled={saving || !name || !baseUrl} onClick={handleSave}>
          保存
        </Button>
      </DialogActions>
    </Dialog>
  );
}

export default function ModelSettingsPage(): React.JSX.Element {
  const {
    providers,
    policies,
    costReport,
    fetchProviders,
    fetchPolicies,
    fetchCostReport,
    upsertProvider,
    deleteProvider,
    upsertPolicy,
  } = useSettingsStore();
  const notify = useAppStore((s) => s.notify);
  const [tab, setTab] = useState(0);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<ModelProvider | null>(null);
  const [testingId, setTestingId] = useState<number | null>(null);
  const [savingPolicy, setSavingPolicy] = useState(false);

  useEffect(() => {
    void fetchProviders();
    void fetchPolicies();
    void fetchCostReport(30);
  }, [fetchProviders, fetchPolicies, fetchCostReport]);

  const testProvider = async (id: number) => {
    setTestingId(id);
    try {
      const result = await modelApi.providerTest(id);
      notify({ severity: "success", message: result });
    } catch {
      /* client 已弹错 */
    } finally {
      setTestingId(null);
    }
  };

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 700 }}>
        模型与 AI 设置
      </Typography>
      <Tabs value={tab} onChange={(_, v) => setTab(v)} sx={{ mb: 2 }}>
        <Tab label="模型接入" />
        <Tab label="路由策略" />
        <Tab label="成本面板" />
      </Tabs>

      {/* 供应商 */}
      {tab === 0 && (
        <Paper sx={{ p: 2 }}>
          <Button
            variant="contained"
            size="small"
            onClick={() => {
              setEditing(null);
              setDialogOpen(true);
            }}
          >
            新增供应商
          </Button>
          <Table size="small" sx={{ mt: 1 }}>
            <TableHead>
              <TableRow>
                <TableCell>名称</TableCell>
                <TableCell>协议</TableCell>
                <TableCell>Base URL</TableCell>
                <TableCell>模型数</TableCell>
                <TableCell>密钥</TableCell>
                <TableCell>启用</TableCell>
                <TableCell>操作</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {providers.map((p) => (
                <TableRow key={p.id}>
                  <TableCell>{p.name}</TableCell>
                  <TableCell>{p.protocol}</TableCell>
                  <TableCell sx={{ maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis" }}>
                    {p.baseUrl}
                  </TableCell>
                  <TableCell>{p.models.length}</TableCell>
                  <TableCell>
                    <Chip
                      size="small"
                      color={p.hasKey ? "success" : "error"}
                      label={p.hasKey ? "已配置" : "未配置"}
                    />
                  </TableCell>
                  <TableCell>
                    <Switch
                      size="small"
                      checked={p.enabled}
                      onChange={async (e) => {
                        await upsertProvider({
                          id: p.id,
                          name: p.name,
                          protocol: p.protocol,
                          baseUrl: p.baseUrl,
                          models: p.models,
                          enabled: e.target.checked,
                          priority: p.priority,
                        });
                      }}
                    />
                  </TableCell>
                  <TableCell>
                    <IconButton
                      size="small"
                      title="连通性测试"
                      disabled={testingId === p.id}
                      onClick={() => testProvider(p.id)}
                    >
                      <NetworkCheckIcon fontSize="small" />
                    </IconButton>
                    <IconButton
                      size="small"
                      title="轮转密钥"
                      onClick={() => {
                        const key = window.prompt(`为 ${p.name} 输入新的 API Key（将写入钥匙串新版本）：`);
                        if (key) {
                          void modelApi.keyRotate(p.keyAlias, key).then((v) =>
                            notify({ severity: "success", message: `密钥已轮转至 v${v}` })
                          );
                        }
                      }}
                    >
                      <KeyIcon fontSize="small" />
                    </IconButton>
                    <IconButton
                      size="small"
                      title="编辑"
                      onClick={() => {
                        setEditing(p);
                        setDialogOpen(true);
                      }}
                    >
                      ✏️
                    </IconButton>
                    <IconButton
                      size="small"
                      title="删除"
                      onClick={() => {
                        if (window.confirm(`删除供应商 ${p.name}？其钥匙串密钥将一并删除。`)) {
                          void deleteProvider(p.id);
                        }
                      }}
                    >
                      <DeleteIcon fontSize="small" />
                    </IconButton>
                  </TableCell>
                </TableRow>
              ))}
              {providers.length === 0 && (
                <TableRow>
                  <TableCell colSpan={7} align="center">
                    <Typography variant="body2" color="text.secondary" sx={{ py: 3 }}>
                      尚未配置供应商。推荐：DeepSeek（推理）+ GLM（低成本）+ Kimi（长文本）
                    </Typography>
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </Paper>
      )}

      {/* 路由策略 */}
      {tab === 1 && (
        <Stack spacing={2}>
          {policies.map((policy) => (
            <ModelRouteForm
              key={policy.taskType}
              policy={policy}
              saving={savingPolicy}
              onSave={async (p) => {
                setSavingPolicy(true);
                try {
                  await upsertPolicy(p);
                  notify({ severity: "success", message: `策略 ${p.taskType} 已保存` });
                } finally {
                  setSavingPolicy(false);
                }
              }}
            />
          ))}
        </Stack>
      )}

      {/* 成本面板 */}
      {tab === 2 && costReport && (
        <Box>
          <Stack direction="row" spacing={2} sx={{ mb: 2 }}>
            <Paper sx={{ p: 2, flex: 1 }}>
              <Typography variant="h5" color="primary">
                {fmtCost(costReport.totalCostCny)}
              </Typography>
              <Typography variant="caption">近 30 天总成本</Typography>
            </Paper>
            <Paper sx={{ p: 2, flex: 1 }}>
              <Typography variant="h5">{costReport.totalCalls}</Typography>
              <Typography variant="caption">调用次数</Typography>
            </Paper>
            <Paper sx={{ p: 2, flex: 1 }}>
              <Typography variant="h5">
                {(costReport.totalTokensIn + costReport.totalTokensOut).toLocaleString()}
              </Typography>
              <Typography variant="caption">Token 总量</Typography>
            </Paper>
          </Stack>
          <Paper sx={{ p: 2, mb: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              按供应商
            </Typography>
            <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
              {costReport.byProvider.map((b) => (
                <Chip key={b.provider} label={`${b.provider}: ${fmtCost(b.costCny)}（${b.calls} 次）`} />
              ))}
            </Stack>
          </Paper>
          <Paper sx={{ p: 2 }}>
            <Typography variant="subtitle2" sx={{ mb: 1 }}>
              最近调用（已脱敏，仅存哈希）
            </Typography>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>时间</TableCell>
                  <TableCell>供应商/模型</TableCell>
                  <TableCell>任务</TableCell>
                  <TableCell>Token</TableCell>
                  <TableCell>成本</TableCell>
                  <TableCell>状态</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {costReport.recentLogs.slice(0, 20).map((log) => (
                  <TableRow key={log.id}>
                    <TableCell>{fmtDateTime(log.createdAt)}</TableCell>
                    <TableCell>
                      {log.provider}/{log.model}
                    </TableCell>
                    <TableCell>{log.taskType}</TableCell>
                    <TableCell>
                      {log.tokensIn}+{log.tokensOut}
                    </TableCell>
                    <TableCell>{fmtCost(log.costCny)}</TableCell>
                    <TableCell>
                      <Chip
                        size="small"
                        color={log.status === "ok" ? "success" : "warning"}
                        label={log.status}
                      />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </Paper>
        </Box>
      )}

      <Divider sx={{ my: 2 }} />
      <ProviderDialog
        open={dialogOpen}
        provider={editing}
        onClose={() => setDialogOpen(false)}
        onSave={upsertProvider}
      />
    </Box>
  );
}
