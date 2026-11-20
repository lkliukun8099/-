// 应用根组件：路由 + 全局布局（侧边导航 + 离线徽章 + 全局通知）
import React, { useEffect, useMemo } from "react";
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import {
  Box,
  CssBaseline,
  Drawer,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  AppBar,
  Toolbar,
  Typography,
  Chip,
  ThemeProvider,
  Snackbar,
  Alert,
  Divider,
} from "@mui/material";
import SmartToyIcon from "@mui/icons-material/SmartToy";
import HomeIcon from "@mui/icons-material/Home";
import PhotoCameraIcon from "@mui/icons-material/PhotoCamera";
import UploadFileIcon from "@mui/icons-material/UploadFile";
import MenuBookIcon from "@mui/icons-material/MenuBook";
import FitnessCenterIcon from "@mui/icons-material/FitnessCenter";
import PrintIcon from "@mui/icons-material/Print";
import InsightsIcon from "@mui/icons-material/Insights";
import ModelTrainingIcon from "@mui/icons-material/ModelTraining";
import SettingsIcon from "@mui/icons-material/Settings";
import MergeTypeIcon from "@mui/icons-material/MergeType";
import { createAppTheme } from "./theme";
import { useAppStore } from "./stores/appStore";
import HomePage from "./pages/HomePage";
import CapturePage from "./pages/CapturePage";
import ImportPage from "./pages/ImportPage";
import MistakeListPage from "./pages/MistakeListPage";
import MistakeDetailPage from "./pages/MistakeDetailPage";
import TrainPage from "./pages/TrainPage";
import PaperPage from "./pages/PaperPage";
import ReportPage from "./pages/ReportPage";
import ModelSettingsPage from "./pages/ModelSettingsPage";
import SystemSettingsPage from "./pages/SystemSettingsPage";
import ConflictMergePage from "./pages/ConflictMergePage";
import ChatPage from "./pages/ChatPage";

const DRAWER_WIDTH = 216;

/** 侧边导航项定义 */
const NAV_ITEMS = [
  { path: "/", label: "今日面板", icon: <HomeIcon /> },
  { path: "/chat", label: "对话助手", icon: <SmartToyIcon /> },
  { path: "/capture", label: "拍照录入", icon: <PhotoCameraIcon /> },
  { path: "/import", label: "相册导入", icon: <UploadFileIcon /> },
  { path: "/mistakes", label: "错题本", icon: <MenuBookIcon /> },
  { path: "/train", label: "今日训练", icon: <FitnessCenterIcon /> },
  { path: "/paper", label: "组卷打印", icon: <PrintIcon /> },
  { path: "/report", label: "诊断报告", icon: <InsightsIcon /> },
  { path: "/conflicts", label: "冲突合并", icon: <MergeTypeIcon /> },
  { path: "/settings/model", label: "模型设置", icon: <ModelTrainingIcon /> },
  { path: "/settings/system", label: "系统设置", icon: <SettingsIcon /> },
];

export default function App(): React.JSX.Element {
  const navigate = useNavigate();
  const location = useLocation();
  const { online, darkMode, notice, closeNotice, initOnlineListener } =
    useAppStore();

  const theme = useMemo(
    () => createAppTheme(darkMode ? "dark" : "light"),
    [darkMode]
  );

  // 启动时监听在线/离线状态（tauri 事件 + 浏览器信号双通道）
  useEffect(() => {
    void initOnlineListener();
  }, [initOnlineListener]);

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <Box sx={{ display: "flex", height: "100%" }}>
        <AppBar
          position="fixed"
          color="default"
          sx={{ zIndex: (t) => t.zIndex.drawer + 1 }}
        >
          <Toolbar variant="dense">
            <Typography variant="h6" sx={{ flexGrow: 1, fontWeight: 700 }}>
              麦兜错题软件
            </Typography>
            {!online && (
              <Chip
                size="small"
                color="warning"
                label="本地模式（离线）"
                sx={{ mr: 1 }}
              />
            )}
          </Toolbar>
        </AppBar>
        <Drawer
          variant="permanent"
          sx={{
            width: DRAWER_WIDTH,
            flexShrink: 0,
            "& .MuiDrawer-paper": { width: DRAWER_WIDTH, boxSizing: "border-box" },
          }}
        >
          <Toolbar variant="dense" />
          <Box sx={{ overflow: "auto" }}>
            <List dense>
              {NAV_ITEMS.map((item) => (
                <ListItemButton
                  key={item.path}
                  selected={
                    item.path === "/"
                      ? location.pathname === "/"
                      : location.pathname.startsWith(item.path)
                  }
                  onClick={() => navigate(item.path)}
                >
                  <ListItemIcon sx={{ minWidth: 36 }}>{item.icon}</ListItemIcon>
                  <ListItemText primary={item.label} />
                </ListItemButton>
              ))}
            </List>
            <Divider />
          </Box>
        </Drawer>
        <Box
          component="main"
          sx={{ flexGrow: 1, p: 2, overflow: "auto", height: "100%" }}
        >
          <Toolbar variant="dense" />
          <Routes>
            <Route path="/" element={<HomePage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/capture" element={<CapturePage />} />
            <Route path="/import" element={<ImportPage />} />
            <Route path="/mistakes" element={<MistakeListPage />} />
            <Route path="/mistakes/:id" element={<MistakeDetailPage />} />
            <Route path="/train" element={<TrainPage />} />
            <Route path="/paper" element={<PaperPage />} />
            <Route path="/report" element={<ReportPage />} />
            <Route path="/conflicts" element={<ConflictMergePage />} />
            <Route path="/settings/model" element={<ModelSettingsPage />} />
            <Route path="/settings/system" element={<SystemSettingsPage />} />
          </Routes>
        </Box>
      </Box>
      {/* 全局通知（api/client 统一弹错也走这里） */}
      <Snackbar
        open={notice !== null}
        autoHideDuration={5000}
        onClose={closeNotice}
        anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
      >
        <Alert
          onClose={closeNotice}
          severity={notice?.severity ?? "info"}
          variant="filled"
        >
          {notice?.message ?? ""}
        </Alert>
      </Snackbar>
    </ThemeProvider>
  );
}
