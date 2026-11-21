# Azure Trusted Signing 云端代码签名配置指南

> 目标：让 maidoupig 安装包通过 Windows 智能应用控制（SAC）信任校验。
> CI 侧已配好（`.github/workflows/ci-build.yml` 的「代码签名」步骤），
> 下面 4 步需要在 Azure 门户由账号持有人完成（约 30 分钟操作 + 1~3 天身份审核）。

## 老爸操作清单

### 第 1 步：注册 Azure 账户（10 分钟）
1. 打开 https://portal.azure.com ，用微软账号登录（没有就先注册，免费）
2. 创建订阅：搜索「订阅」→ 添加 → 选**即用即付**（需绑一张 Visa/ Mastercard 信用卡，签名服务个人档约 $10/月量级，以官网价格为准）

### 第 2 步：创建 Trusted Signing 账户（5 分钟）
1. 搜索栏输入「Trusted Signing 账户」→ 创建
2. 资源组：新建 `maidoupig-rg`；账户名：`maidoupig-sign`（全局唯一，冲突就换）
3. 区域：选 **东亚（East Asia）** 或 **东南亚（Southeast Asia）**
4. 定价层：**Basic**（个人/小团队够用）
5. 记下「概述」页的 **账户 URI（endpoint）**，形如 `https://easia.codesigning.azure.net`

### 第 3 步：身份验证 + 证书配置文件（审核 1~3 天）
1. 在刚创建的账户里 → 「身份验证」→ 新建：
   - **推荐企业身份**：南京釜鼎新能源科技有限公司（填统一社会信用代码，传营业执照，审核快、信誉等级高）
   - 个人身份：上传身份证/护照（部分区域仍受限，企业身份更稳）
2. 审核通过后 → 「证书配置文件」→ 创建：
   - 配置文件名：`maidoupig-release`
   - 类型：**Public Trust**（公开信任，SAC/SmartScreen 认可）

### 第 4 步：创建服务主体并授权（10 分钟）
1. 搜索「Microsoft Entra ID」→ 「应用注册」→ 新注册：名称 `maidoupig-ci`，其余默认
2. 记下：**应用程序(客户端) ID** 和 **目录(租户) ID**
3. 「证书和机密」→ 新建客户端机密 → 记下 **机密值**（只显示一次！）
4. 回到 Trusted Signing 账户 → 「访问控制(IAM)」→ 添加角色分配：
   - 角色：**Trusted Signing Certificate Profile Signer**
   - 成员：选 `maidoupig-ci` 应用

### 第 5 步：把 6 个值发给我（或自己填到 GitHub）
在仓库 https://github.com/lkliukun8099/-/settings/secrets/actions 配置：

**Secrets（3 个）**：
| 名称 | 值 |
|---|---|
| `AZURE_TENANT_ID` | 第 4 步的目录(租户) ID |
| `AZURE_CLIENT_ID` | 第 4 步的应用程序(客户端) ID |
| `AZURE_CLIENT_SECRET` | 第 4 步的客户端机密值 |

**Variables（3 个，在 Variables 标签页）**：
| 名称 | 值 |
|---|---|
| `TS_ACCOUNT_NAME` | `maidoupig-sign` |
| `TS_ENDPOINT` | 第 2 步的账户 URI |
| `TS_CERT_PROFILE` | `maidoupig-release` |

配好后告诉我，我推送一个空提交触发 CI，安装包就会带受信任签名，SAC 直接放行。

## 原理说明
- CI 在 `tauri build` 后用 `azure/trusted-signing-action` 对 `target/release` 下全部 `.exe`（含 NSIS 安装包主程序和 maidoupig.exe）批量签名
- 签名证书由微软云托管，私钥不出云，不存在证书泄露风险
- 未配置 secrets 时签名步骤自动跳过，流水线照常产出未签名包，不影响现有流程
