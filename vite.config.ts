import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Vite 配置（含 Tauri 2 集成约定：固定端口 1420，禁止清屏以便查看 Rust 侧输出）
export default defineConfig(async () => ({
  plugins: [react()],
  // 防止 Vite 遮蔽 Rust 编译错误
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // 不监听 src-tauri，避免 Rust 构建触发前端热更新循环
      ignored: ["**/src-tauri/**"],
    },
  },
  // Tauri 期望以相对路径加载前端资源
  base: "./",
  build: {
    // Tauri 在 Windows 上使用 Chromium，在 macOS/Linux 上使用 WebKit
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  worker: {
    format: "es",
  },
}));
