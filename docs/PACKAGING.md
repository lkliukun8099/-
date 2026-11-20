# 打包说明

## v0.1.0 轻量包（当前）
- `tauri.conf.json` 不含 `externalBin`，tesseract / mathverify sidecar 不随包分发，运行时检测缺失自动降级（离线 OCR / SymPy 校验跳过）
- CI：`npm run tauri build`（MSVC + NASM，见 .github/workflows/ci-build.yml）

## 完整包（后续）
1. 放置 tesseract sidecar 至 `src-tauri/binaries/`、PyInstaller 打包 mathverify 至 `src-tauri/tools/mathverify/`
2. 恢复 bundle 配置：`"externalBin": ["binaries/tesseract", "tools/mathverify/mathverify"]`
3. 注意：tauri.conf.json 为严格 JSON，不支持注释

## MSI（可选）
WiX 默认代码页 1252 不支持中文 productName（LGHT0311）。如需 MSI：恢复 targets 含 "msi"，并配置 `bundle.windows.wix.language = "zh-CN"`（代码页 936）。
