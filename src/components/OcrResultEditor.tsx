// OCR 结果可编辑组件：纯文本 + LaTeX 双栏编辑，右侧实时 KaTeX 预览
import React, { useState } from "react";
import { Box, Button, Stack, TextField, Typography } from "@mui/material";
import LatexText from "./LatexText";

interface OcrResultEditorProps {
  initialText: string;
  initialLatex: string;
  saving?: boolean;
  onSave: (text: string, latex: string) => void | Promise<void>;
}

export default function OcrResultEditor({
  initialText,
  initialLatex,
  saving = false,
  onSave,
}: OcrResultEditorProps): React.JSX.Element {
  const [text, setText] = useState(initialText);
  const [latex, setLatex] = useState(initialLatex);
  const [editing, setEditing] = useState(false);

  const handleSave = async () => {
    await onSave(text, latex);
    setEditing(false);
  };

  if (!editing) {
    return (
      <Box>
        <Box className="selectable" sx={{ p: 1.5, bgcolor: "action.hover", borderRadius: 1 }}>
          <LatexText text={text || "（暂无 OCR 文本）"} />
          {latex && (
            <Box sx={{ mt: 1 }}>
              <Typography variant="caption" color="text.secondary">
                LaTeX：
              </Typography>
              <LatexText text={`$$${latex}$$`} />
            </Box>
          )}
        </Box>
        <Button size="small" sx={{ mt: 1 }} onClick={() => setEditing(true)}>
          编辑识别结果
        </Button>
      </Box>
    );
  }

  return (
    <Box>
      <Stack direction={{ xs: "column", md: "row" }} spacing={2}>
        <Box sx={{ flex: 1 }}>
          <TextField
            label="OCR 文本"
            multiline
            minRows={4}
            fullWidth
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
          <TextField
            label="LaTeX 公式"
            multiline
            minRows={2}
            fullWidth
            sx={{ mt: 1.5 }}
            value={latex}
            onChange={(e) => setLatex(e.target.value)}
          />
        </Box>
        <Box sx={{ flex: 1, p: 1.5, bgcolor: "action.hover", borderRadius: 1 }}>
          <Typography variant="caption" color="text.secondary">
            预览
          </Typography>
          <LatexText text={text} />
          {latex && <LatexText text={`$$${latex}$$`} />}
        </Box>
      </Stack>
      <Stack direction="row" spacing={1} sx={{ mt: 1.5 }}>
        <Button size="small" variant="contained" disabled={saving} onClick={handleSave}>
          保存
        </Button>
        <Button size="small" onClick={() => setEditing(false)}>
          取消
        </Button>
      </Stack>
    </Box>
  );
}
