// KaTeX 渲染封装：文本中的 $...$ / $$...$$ 公式自动渲染，其余原样显示
import React from "react";
import { InlineMath, BlockMath } from "react-katex";

interface LatexTextProps {
  text: string;
  className?: string;
}

/** 将含 LaTeX 定界符的文本切分为 [文本, 行内公式, 块级公式] 片段 */
function splitSegments(text: string): Array<{ type: "text" | "inline" | "block"; value: string }> {
  const segments: Array<{ type: "text" | "inline" | "block"; value: string }> = [];
  // 先按 $$ 切，再按 $ 切
  const blockParts = text.split("$$");
  blockParts.forEach((part, i) => {
    if (i % 2 === 1) {
      segments.push({ type: "block", value: part });
      return;
    }
    const inlineParts = part.split("$");
    inlineParts.forEach((sub, j) => {
      if (j % 2 === 1) {
        segments.push({ type: "inline", value: sub });
      } else if (sub) {
        segments.push({ type: "text", value: sub });
      }
    });
  });
  return segments;
}

/** 公式渲染失败时回退为原文（KaTeX throwOnError=false 也可，这里双保险） */
function SafeMath({ type, value }: { type: "inline" | "block"; value: string }): React.JSX.Element {
  try {
    return type === "inline" ? <InlineMath math={value} /> : <BlockMath math={value} />;
  } catch {
    return <code>{value}</code>;
  }
}

export default function LatexText({ text, className }: LatexTextProps): React.JSX.Element {
  const segments = splitSegments(text);
  return (
    <span className={className} style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
      {segments.map((seg, idx) =>
        seg.type === "text" ? (
          <React.Fragment key={idx}>{seg.value}</React.Fragment>
        ) : (
          <SafeMath key={idx} type={seg.type} value={seg.value} />
        )
      )}
    </span>
  );
}
