// 今日训练：今日复习队列（自评推进）+ 变式作答（判分即时反馈，F-15 闭环）
import React, { useEffect, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Chip,
  Paper,
  Stack,
  Tab,
  Tabs,
  TextField,
  Typography,
} from "@mui/material";
import { reviewApi } from "../api/review";
import { useTrainStore } from "../stores/trainStore";
import { useAppStore } from "../stores/appStore";
import LatexText from "../components/LatexText";
import type { ReviewItem } from "../types";
import { variantTypeText, difficultyText } from "../utils/format";

/** 复习卡片：展示题干，自评对错推进排程 */
function ReviewCard({
  item,
  onDone,
}: {
  item: ReviewItem;
  onDone: () => void;
}): React.JSX.Element {
  const [submitting, setSubmitting] = useState(false);
  const complete = async (correct: boolean) => {
    setSubmitting(true);
    try {
      await reviewApi.complete(item.plan.id, correct);
      onDone();
    } finally {
      setSubmitting(false);
    }
  };
  return (
    <Paper sx={{ p: 2, mb: 1.5 }}>
      <Stack direction="row" spacing={1} alignItems="center" sx={{ mb: 1 }}>
        <Chip size="small" label={item.plan.targetType === "mistake" ? "原错题" : "变式题"} />
        <Typography variant="caption" color="text.secondary">
          复习阶段 {item.plan.stage}
        </Typography>
      </Stack>
      <Box className="selectable">
        <LatexText text={item.title} />
      </Box>
      <Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 1 }}>
        请先在纸上作答并核对，然后自评：
      </Typography>
      <Stack direction="row" spacing={1} sx={{ mt: 1 }}>
        <Button size="small" variant="contained" color="success" disabled={submitting} onClick={() => complete(true)}>
          答对了
        </Button>
        <Button size="small" variant="outlined" color="error" disabled={submitting} onClick={() => complete(false)}>
          答错了
        </Button>
      </Stack>
    </Paper>
  );
}

/** 变式作答卡片 */
function VariantCard({ variantId }: { variantId: number }): React.JSX.Element {
  const { queue, feedbacks, submitAnswer, submitting } = useTrainStore();
  const variant = queue.find((v) => v.id === variantId);
  const [answer, setAnswer] = useState("");
  const feedback = feedbacks[variantId];
  if (!variant) return <></>;

  return (
    <Paper sx={{ p: 2, mb: 1.5 }}>
      <Stack direction="row" spacing={1} alignItems="center" sx={{ mb: 1 }}>
        <Chip size="small" label={variantTypeText(variant.variantType)} />
        <Typography variant="caption" color="warning.main">
          {difficultyText(variant.difficulty)}
        </Typography>
      </Stack>
      <Box className="selectable">
        <LatexText text={variant.content} />
        {variant.contentLatex && <LatexText text={`$$${variant.contentLatex}$$`} />}
      </Box>
      {!feedback ? (
        <Stack direction="row" spacing={1} sx={{ mt: 1.5 }}>
          <TextField
            size="small"
            label="你的答案"
            sx={{ flex: 1 }}
            value={answer}
            onChange={(e) => setAnswer(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && answer.trim()) {
                void submitAnswer(variantId, answer.trim());
              }
            }}
          />
          <Button
            variant="contained"
            disabled={submitting || !answer.trim()}
            onClick={() => submitAnswer(variantId, answer.trim())}
          >
            提交
          </Button>
        </Stack>
      ) : (
        <Alert severity={feedback.isCorrect ? "success" : "error"} sx={{ mt: 1.5 }}>
          <LatexText text={feedback.feedback} />
          {feedback.masteryDelta !== null && (
            <Typography variant="caption" display="block" sx={{ mt: 0.5 }}>
              掌握度变化：{feedback.masteryDelta >= 0 ? "+" : ""}
              {(feedback.masteryDelta * 100).toFixed(1)}%
            </Typography>
          )}
        </Alert>
      )}
    </Paper>
  );
}

export default function TrainPage(): React.JSX.Element {
  const [tab, setTab] = useState(0);
  const [reviews, setReviews] = useState<ReviewItem[]>([]);
  const { queue, queueLoading, loadQueue } = useTrainStore();
  const notify = useAppStore((s) => s.notify);

  const loadReviews = async () => {
    try {
      setReviews(await reviewApi.today());
    } catch {
      /* client 已弹错 */
    }
  };

  useEffect(() => {
    void loadReviews();
    void loadQueue();
  }, [loadQueue]);

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 700 }}>
        今日训练
      </Typography>
      <Tabs value={tab} onChange={(_, v) => setTab(v)} sx={{ mb: 2 }}>
        <Tab label={`今日复习（${reviews.length}）`} />
        <Tab label={`变式训练（${queue.length}）`} />
      </Tabs>

      {tab === 0 &&
        (reviews.length === 0 ? (
          <Paper sx={{ p: 4, textAlign: "center" }}>
            <Typography color="text.secondary">今日没有到期复习，继续保持！</Typography>
          </Paper>
        ) : (
          reviews.map((item) => (
            <ReviewCard
              key={item.plan.id}
              item={item}
              onDone={() => {
                notify({ severity: "success", message: "复习进度已更新" });
                void loadReviews();
              }}
            />
          ))
        ))}

      {tab === 1 &&
        (queue.length === 0 && !queueLoading ? (
          <Paper sx={{ p: 4, textAlign: "center" }}>
            <Typography color="text.secondary">
              暂无可训练的变式题，去错题详情页生成变式题吧
            </Typography>
          </Paper>
        ) : (
          queue.map((v) => <VariantCard key={v.id} variantId={v.id} />)
        ))}
    </Box>
  );
}
