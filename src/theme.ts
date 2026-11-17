// MUI 主题：中文排版优化，支持亮/暗两种模式
import { createTheme, type Theme } from "@mui/material/styles";

/**
 * 创建应用主题。
 * 中文字体栈优先使用系统字体，保证 Windows/macOS 下渲染一致。
 */
export function createAppTheme(mode: "light" | "dark" = "light"): Theme {
  return createTheme({
    palette: {
      mode,
      primary: {
        main: "#ea580c", // 麦兜暖橙
      },
      secondary: {
        main: "#0ea5e9",
      },
      success: { main: "#16a34a" },
      warning: { main: "#d97706" },
      error: { main: "#dc2626" },
    },
    typography: {
      fontFamily: [
        "-apple-system",
        "BlinkMacSystemFont",
        '"Segoe UI"',
        '"PingFang SC"',
        '"Hiragino Sans GB"',
        '"Microsoft YaHei"',
        "sans-serif",
      ].join(","),
      // 中文排版：正文行高略大，提升可读性
      body1: { lineHeight: 1.75 },
      body2: { lineHeight: 1.7 },
    },
    shape: { borderRadius: 10 },
    components: {
      MuiButton: {
        defaultProps: { disableElevation: true },
      },
      MuiPaper: {
        defaultProps: { elevation: 0, variant: "outlined" },
      },
    },
  });
}
