/** @type {import('tailwindcss').Config} */
// Tailwind 配置：与 MUI 共存（关闭 preflight 中与 MUI 冲突的部分通过 important 策略处理）
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  // 避免 Tailwind preflight 重置 MUI 组件样式
  corePlugins: {
    preflight: false,
  },
  theme: {
    extend: {
      colors: {
        // 品牌色：麦兜暖橙
        brand: {
          50: "#fff7ed",
          100: "#ffedd5",
          500: "#f97316",
          600: "#ea580c",
          700: "#c2410c",
        },
      },
    },
  },
  plugins: [],
};
