// 组卷打印（P1 路由骨架，F-18）
import React from "react";
import { Box, Paper, Typography } from "@mui/material";
import PrintIcon from "@mui/icons-material/Print";

export default function PaperPage(): React.JSX.Element {
  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 2, fontWeight: 700 }}>
        组卷打印
      </Typography>
      <Paper sx={{ p: 6, textAlign: "center" }}>
        <PrintIcon sx={{ fontSize: 56, color: "text.disabled" }} />
        <Typography variant="h6" sx={{ mt: 2 }}>
          P1 版本功能（预留）
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mt: 1, maxWidth: 480, mx: "auto" }}>
          后续版本将支持：按掌握度/标签/知识点筛选组卷、导出 Word/PDF、打印排版。
          当前版本可在「错题本」中筛选后使用系统打印。
        </Typography>
      </Paper>
    </Box>
  );
}
