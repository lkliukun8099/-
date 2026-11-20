# 打包说明

## v0.1.0 轻量包（当前）
- `tauri.conf.json` 不含 `externalBin`，tesseract / mathverify sidecar 不随包分发，运行时检测缺失自动降级（离线 OCR / SymPy 校验跳过）
- CI：`npm run tauri build`（MSVC + NASM，见 .github/workflows/ci-build.yml）

## 完整包（后续）
1. 放置 tesseract sidecar 至 `src-tauri/binaries/`、PyInstaller 打包 mathverify 至 `src-tauri/tools/mathverify/`
2. 恢复 bundle 配置：`"externalBin": ["binaries/tesseract", "tools/mathverify/mathverify"]`
3. 注意：tauri.conf.json 为严格 JSON，不支持注释
