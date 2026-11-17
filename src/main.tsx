// React 挂载入口：主题 + 路由 + 全局状态初始化
import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import App from "./App";
import "./styles/index.css";
import "katex/dist/katex.min.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {/* Tauri WebView 内使用 HashRouter，避免文件协议下的路由问题 */}
    <HashRouter>
      <App />
    </HashRouter>
  </React.StrictMode>
);
