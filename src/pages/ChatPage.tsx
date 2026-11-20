// 对话式错题分析页（聊天入口）：输入错题 → 错误点诊断 → 指正 → 变式训练
import React, { useRef, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Collapse,
  Divider,
  IconButton,
  Paper,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import SendIcon from "@mui/icons-material/Send";
import SmartToyIcon from "@mui/icons-material/SmartToy";
import PersonIcon from "@mui/icons-material/Person";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import { useNavigate } from "react-router-dom";
import { chatApi } from "../api/chat";
import type { ChatAnalyzeResult } from "../types";
import LatexText from "../components/LatexText";

interface ChatTurn {
  question: string;
  myAnswer?: string;
  result?: ChatAnalyzeResult;
  error?: string;
}

const EXAMPLES = [
  "已知函数f(x)=x²-2ax+3在区间[1,2]上单调递减，求a的取值范围",
  "物块静止在粗糙斜面上，分析它受到的摩擦力方向",
  "求函数 y=√(x+2)/(x-1) 的定义域",
];

export default function ChatPage(): React.JSX.Element {
  const navigate = useNavigate();
  const [turns, setTurns] = useState<ChatTurn[]>([]);
  const [question, setQuestion] = useState("");
  const [myAnswer, setMyAnswer] = useState("");
  const [busy, setBusy] = useState(false);
  const [showAnswer, setShowAnswer] = useState<Record<string, boolean>>({});
  const bottomRef = useRef<HTMLDivElement>(null);

  const scrollDown = () =>
    setTimeout(() => bottomRef.current?.scrollIntoView({ behavior: "smooth" }), 60);

  const send = async () => {
    const q = question.trim();
    if (q.length < 4 || busy) return;
    const a = myAnswer.trim();
    setTurns((t) => [...t, { question: q, myAnswer: a || undefined }]);
    setQuestion("");
    setMyAnswer("");
    setBusy(true);
    scrollDown();
    try {
      const result = await chatApi.analyze(q, a || undefined);
      setTurns((t) => {
        const next = [...t];
        next[next.length - 1] = { ...next[next.length - 1], result };
        return next;
      });
    } catch (e) {
      setTurns((t) => {
        const next = [...t];
        next[next.length - 1] = {
          ...next[next.length - 1],
          error: e instanceof Error ? e.message : String(e),
        };
        return next;
      });
    } finally {
      setBusy(false);
      scrollDown();
    }
  };

  return (
    <Box sx={{ display: "flex", flexDirection: "column", height: "100%", gap: 2 }}>
      <Typography variant="h6" fontWeight={700}>
        对话助手
        <Typography component="span" variant="body2" color="text.secondary" sx={{ ml: 1.5 }}>
          发错题 → 找错因 → 给指正 → 出同类题（自动归档进错题本）
        </Typography>
      </Typography>

      {/* 消息区 */}
      <Box sx={{ flexGrow: 1, overflow: "auto", display: "flex", flexDirection: "column", gap: 2, pr: 1 }}>
        {turns.length === 0 && (
          <Paper variant="outlined" sx={{ p: 2.5 }}>
            <Stack direction="row" spacing={1} alignItems="center" sx={{ mb: 1 }}>
              <SmartToyIcon color="primary" />
              <Typography fontWeight={700}>你好，我是麦兜</Typography>
            </Stack>
            <Typography variant="body2" color="text.secondary" sx={{ lineHeight: 1.9 }}>
              把做错的题目发给我（最好附上你的错误作答），我会列出<b>具体错误点</b>、给出<b>指正与正确解法</b>，
              并出 <b>2~3 道同类变式题</b>。错题会自动归档到错题本，变式题进今日训练队列。
            </Typography>
            <Stack direction="row" spacing={1} sx={{ mt: 1.5, flexWrap: "wrap" }} useFlexGap>
              {EXAMPLES.map((e) => (
                <Chip key={e} label={e.slice(0, 16) + "…"} variant="outlined" size="small"
                  onClick={() => setQuestion(e)} sx={{ cursor: "pointer" }} />
              ))}
            </Stack>
          </Paper>
        )}

        {turns.map((turn, i) => (
          <React.Fragment key={i}>
            {/* 用户消息 */}
            <Stack direction="row" spacing={1} justifyContent="flex-end">
              <Paper sx={{ p: 1.5, maxWidth: "80%", bgcolor: "primary.main", color: "primary.contrastText" }}>
                <Typography variant="body2" sx={{ whiteSpace: "pre-wrap" }}>{turn.question}</Typography>
                {turn.myAnswer && (
                  <Typography variant="caption" sx={{ display: "block", mt: 0.5, opacity: 0.85 }}>
                    ✏️ 我的作答：{turn.myAnswer}
                  </Typography>
                )}
              </Paper>
              <PersonIcon sx={{ mt: 0.5 }} />
            </Stack>

            {/* 分析结果 */}
            {!turn.result && !turn.error && (
              <Stack direction="row" spacing={1} alignItems="center">
                <SmartToyIcon color="action" />
                <CircularProgress size={16} />
                <Typography variant="caption" color="text.secondary">麦兜正在分析…</Typography>
              </Stack>
            )}
            {turn.error && <Alert severity="error" sx={{ maxWidth: "85%" }}>{turn.error}</Alert>}
            {turn.result && (
              <Stack direction="row" spacing={1} alignItems="flex-start">
                <SmartToyIcon color="primary" sx={{ mt: 0.5 }} />
                <Stack spacing={1.5} sx={{ maxWidth: "85%", flexGrow: 1 }}>
                  {/* 错误点 */}
                  <Card variant="outlined">
                    <CardContent sx={{ py: 1.5, "&:last-child": { pb: 1.5 } }}>
                      <Typography variant="subtitle2" color="error" gutterBottom>🔍 错误点诊断</Typography>
                      {turn.result.attribution ? (
                        <>
                          <Stack direction="row" spacing={1} useFlexGap sx={{ flexWrap: "wrap" }}>
                            <Chip color="error" size="small" label={turn.result.attribution.causeLabel} />
                            {turn.result.attribution.kgPath.length > 0 && (
                              <Chip variant="outlined" size="small"
                                label={`知识定位：${turn.result.attribution.kgPath.join(" / ")}`} />
                            )}
                            <Chip variant="outlined" size="small"
                              label={`引擎：${turn.result.attribution.engine} · 置信度 ${(turn.result.attribution.confidence * 100).toFixed(0)}%`} />
                          </Stack>
                          {turn.result.attribution.causeDetail && (
                            <Typography variant="body2" sx={{ mt: 1, lineHeight: 1.8 }}>
                              <LatexText text={turn.result.attribution.causeDetail} />
                            </Typography>
                          )}
                        </>
                      ) : (
                        <Typography variant="body2" color="text.secondary">归因未产出（可在错题详情页重试）</Typography>
                      )}
                    </CardContent>
                  </Card>

                  {/* 变式训练 */}
                  <Card variant="outlined">
                    <CardContent sx={{ py: 1.5, "&:last-child": { pb: 1.5 } }}>
                      <Typography variant="subtitle2" color="success.main" gutterBottom>
                        💪 同类变式训练（{turn.result.variants.length} 道）
                      </Typography>
                      {turn.result.variants.length === 0 && (
                        <Typography variant="body2" color="text.secondary">
                          {turn.result.note ?? "暂无变式题"}
                        </Typography>
                      )}
                      {turn.result.variants.map((v, j) => {
                        const key = `${i}-${j}`;
                        return (
                          <Paper key={j} variant="outlined" sx={{ p: 1.5, mt: 1, bgcolor: "background.default" }}>
                            <Typography variant="body2" fontWeight={600}>
                              变式 {j + 1}（{v.variantType} · 难度 {v.difficulty}）
                            </Typography>
                            <Typography variant="body2" sx={{ mt: 0.5, lineHeight: 1.8 }}>
                              <LatexText text={v.contentLatex ?? v.content} />
                            </Typography>
                            <Stack direction="row" alignItems="center" spacing={0.5}
                              sx={{ cursor: "pointer", mt: 0.5 }}
                              onClick={() => setShowAnswer((s) => ({ ...s, [key]: !s[key] }))}>
                              <Typography variant="caption" color="primary" fontWeight={700}>
                                {showAnswer[key] ? "收起答案" : "查看答案与解析"}
                              </Typography>
                              <IconButton size="small"
                                sx={{ transform: showAnswer[key] ? "rotate(180deg)" : "none", transition: ".2s" }}>
                                <ExpandMoreIcon fontSize="small" />
                              </IconButton>
                            </Stack>
                            <Collapse in={!!showAnswer[key]}>
                              <Alert severity="success" variant="outlined" sx={{ mt: 0.5 }}>
                                <Typography variant="body2" sx={{ lineHeight: 1.8 }}>
                                  <LatexText text={v.solution ? `${v.answer}\n${v.solution}` : v.answer} />
                                </Typography>
                              </Alert>
                            </Collapse>
                          </Paper>
                        );
                      })}
                      {turn.result.note && turn.result.variants.length > 0 && (
                        <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 1 }}>
                          {turn.result.note}
                        </Typography>
                      )}
                      <Divider sx={{ my: 1 }} />
                      <Button size="small" onClick={() => navigate(`/mistakes/${turn.result!.mistakeId}`)}>
                        在错题本中查看 #{turn.result.mistakeId}
                      </Button>
                    </CardContent>
                  </Card>
                </Stack>
              </Stack>
            )}
          </React.Fragment>
        ))}
        <div ref={bottomRef} />
      </Box>

      {/* 输入区 */}
      <Paper variant="outlined" sx={{ p: 1.5 }}>
        <TextField
          fullWidth multiline minRows={2} maxRows={5} size="small"
          placeholder="把做错的题目输入到这里…（Enter 发送，Shift+Enter 换行）"
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
        />
        <Stack direction="row" spacing={1} sx={{ mt: 1 }} alignItems="center">
          <TextField
            fullWidth size="small" placeholder="（可选）你的错误作答，填了分析更准"
            value={myAnswer}
            onChange={(e) => setMyAnswer(e.target.value)}
          />
          <Button
            variant="contained" endIcon={<SendIcon />} onClick={() => void send()}
            disabled={busy || question.trim().length < 4} sx={{ whiteSpace: "nowrap" }}>
            发送分析
          </Button>
        </Stack>
      </Paper>
    </Box>
  );
}
