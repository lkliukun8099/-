# maidoupig 代码签名完整手册（2026-11 修订版）

> 目标：让 maidoupig 安装包通过 Windows 智能应用控制（SAC）/ SmartScreen 校验。
> CI 侧已预留签名步骤（`.github/workflows/ci-build.yml`），配好密钥后自动生效。

---

## 0. 先读：三条路的现状（2026-11 核实）

| 方案 | 中国实体可用 | 费用 | CI 集成 | 结论 |
|---|---|---|---|---|
| Azure Artifact Signing（原 Trusted Signing） | ❌ 不支持中国公司/中国个人 | $9.99/月 | 最丝滑 | **此路不通**（地域限制） |
| **ssl.com eSigner（OV 云签名）** | ✅ 支持中国企业验证 | ~$129/年 | 官方 GitHub Action | **推荐（付费正道）** |
| **Certum 开源签名** | ✅ 个人身份即可 | **免费** | 手动步骤较多 | **推荐（零成本，契合开源核心）** |
| 传统 CA（DigiCert/Sectigo OV） | ✅ | $150~300/年 | 云 HSM 工具链 | 备选 |

**期望值管理（重要）**：签名 ≠ 立即免警告。SAC/SmartScreen 采用声誉累积制——
同一发布者身份连续签名发布若干版本后，警告才会逐步消失（EV 证书自 2024 年起也不再即时豁免）。
签名解决的是「无法验证发布者」这一硬性拦截，声誉靠持续发布积累。

---

## 方案 A：ssl.com eSigner OV 云签名（推荐·付费正道）

适合：愿意 ~$129/年换省心，企业身份（釜鼎新能源）验证。

### 第 1 步：购买与下单（15 分钟）
1. 打开 https://www.ssl.com/certificates/code-signing/
2. 选 **OV Code Signing Certificate**（1 年，约 $129；多年更便宜）
3. 下单时主体信息填企业：**Nanjing Fuding New Energy Technology Co., Ltd.**（与营业执照一致）
4. 支付方式：信用卡/PayPal

### 第 2 步：企业身份验证（1~5 个工作日）
1. ssl.com 会邮件要求提交：营业执照扫描件、法人身份证明、企业电话
2. 可能有人工电话复核（留真实座机/手机）
3. 验证通过后证书签发至 **eSigner 云 HSM**（私钥不出云，无需 U 盾）

### 第 3 步：开通 eSigner 云签名（10 分钟）
1. 登录 ssl.com 控制台 → 证书详情 → eSigner → 启用
2. 记录三个值：
   - **用户名 / 密码**（ssl.com 账户）
   - **Credential ID**（eSigner 页面显示）
   - **TOTP Secret**（绑定二次验证时显示的密钥串，务必保存）

### 第 4 步：填到 GitHub Secrets（5 分钟）
仓库 → Settings → Secrets and variables → Actions → New repository secret：

| Secret 名称 | 值 |
|---|---|
| `ES_USERNAME` | ssl.com 用户名 |
| `ES_PASSWORD` | ssl.com 密码 |
| `ES_CREDENTIAL_ID` | eSigner Credential ID |
| `ES_TOTP_SECRET` | TOTP 密钥串 |

### 第 5 步：通知我
CI 中 `sslcom/esigner-codesign` 步骤已预埋（密钥存在即自动执行）。
推送任意提交即产出**带 OV 云签名的安装包**。

---

## 方案 B：Certum 开源代码签名（推荐·零成本）【已选定，进行中】

适合：项目开源（我们本就是开源核心路线），不花一分钱。

### ✅ 已完成（2026-11-21）
- 仓库转 Public 并改名：https://github.com/lkliukun8099/maidoupig
- MIT LICENSE 已入库
- CI 已预埋 Certum SimplySign 签名步骤（secrets 配好自动生效）

### 第 1 步：注册 Certum 账户（10 分钟）
1. 打开 https://en.certum.pl → 注册（选 **Individual / 个人开发者**）
2. 邮箱激活 + 设置密码

### 第 2 步：申请 Open Source Code Signing（15 分钟 + 1~3 天审核）
1. 登录后找到 **Open Source Code Signing**（免费档）申请入口
2. 填写开源项目地址：https://github.com/lkliukun8099/maidoupig
3. 个人身份验证：上传身份证或护照照片 + 手机号验证（中国证件支持）
4. 等待审核邮件（1~3 个工作日）

### 第 3 步：激活 SimplySign 云签名（15 分钟）
1. 审核通过后，证书发放至 **SimplySign 云 HSM**（私钥不出云）
2. 手机安装 **SimplySign** App（应用商店搜 SimplySign，扫码绑定）
3. 绑定时会显示 **TOTP 密钥串**（一串 Base32 字符）——**务必抄下来**，
   它同时用于 App 动态口令和 CI 自动签名

### 第 4 步：把 3 个值发我（或自己填 GitHub Secrets）
仓库 → Settings → Secrets and variables → Actions：

| Secret 名称 | 值 |
|---|---|
| `CERTUM_USER` | Certum 账户邮箱 |
| `CERTUM_PASSWORD` | Certum 账户密码 |
| `CERTUM_TOTP_SECRET` | 第 3 步的 TOTP 密钥串 |

### 第 5 步：联调首签
secrets 配好后告诉我，我推送触发 CI，观察签名步骤日志，
必要时按 Certum 最新文档微调 signtool 参数（证书指纹等），
成功后每个版本的安装包都带受信任签名。

---

## 方案 C：传统 CA OV 证书（备选）

DigiCert / Sectigo / GlobalSign 官网或授权代理（如 SignMyCode、cheapsslsecurity）下单，
企业验证流程与方案 A 类似，$150~300/年。签发形式多为云 HSM 或 USB 令牌（令牌不适合 CI，选云 HSM 交付）。

---

## CI 集成说明（已完成）
`.github/workflows/ci-build.yml` 的打包 job 内含两个签名步骤，均**按密钥存在与否自动启用**：
1. `azure/trusted-signing-action`（Azure 路线，中国实体不可用，保留占位）
2. `sslcom/esigner-codesign`（方案 A，配好 4 个 ES_* secrets 即生效）

签名对象：`src-tauri/target/release` 下全部 `.exe`（安装包 + 主程序）。
未配置任何密钥时跳过签名，照常产出未签名包，流水线不报错。

## 签名后验证
下载新安装包 → 右键属性 → 「数字签名」标签页应显示发布者名称；
双击运行，SAC 不再弹「无法验证发布者」硬拦截
（初期可能仍有 SmartScreen 蓝色提示，点「更多信息 → 仍要运行」即可，
连续发布数个签名版本后提示自然消失）。
