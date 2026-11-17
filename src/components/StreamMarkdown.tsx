// 流式增量渲染组件：接收逐块文本，内部累积渲染（含 LaTeX 与降级标记）
import React, { useEffect, useRef, useState } from "react";
import { Box, Chip, CircularProgress } from "@mui/material";
import LatexText from "./LatexText";
import type { StreamChunk } from "../types";

interface StreamMarkdownProps {
  /** 父组件传入的流式分块（可为 null 表示未开始） */
  chunk: StreamChunk | null;
  /** 流结束后是否显示完成态 */
  showStatus?: boolean;
}

/**
 * 用法：父组件在 onChunk 回调中 setState(chunk)，本组件内部累积全文。
 * 用 ref 存全文避免每块重建字符串数组（§9-5：流式数据走 ref 增量）。
 */
export default function StreamMarkdown({
  chunk,
  showStatus = true,
}: StreamMarkdownProps): React.JSX.Element {
  const fullTextRef = useRef("");
  const degradedRef = useRef<string | null>(null);
  const [, forceRender] = useState(0);
  const [done, setDone] = useState(false);

  useEffect(() => {
    if (!chunk) return;
    if (chunk.degraded) degradedRef.current = chunk.degraded;
    if (chunk.delta) {
      fullTextRef.current += chunk.delta;
      forceRender((n) => n + 1);
    }
    if (chunk.done) setDone(true);
  }, [chunk]);

  // 外部重置（chunk 重新为 null 时清空）
  useEffect(() => {
    if (chunk === null) {
      fullTextRef.current = "";
      degradedRef.current = null;
      setDone(false);
      forceRender((n) => n + 1);
    }
  }, [chunk]);

  return (
    <Box>
      {degradedRef.current && (
        <Chip
          size="small"
          color="warning"
          label={`已降级（原模型 ${degradedRef.current} 不可用）`}
          sx={{ mb: 1 }}
        />
      )}
      <Box className="selectable">
        <LatexText text={fullTextRef.current} />
      </Box>
      {showStatus && !done && fullTextRef.current.length > 0 && (
        <CircularProgress size={14} sx={{ ml: 1 }} />
      )}
    </Box>
  );
}
