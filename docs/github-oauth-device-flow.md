# GitHub OAuth App 与 Device Flow 配置指南

本文档说明如何为「云同步」的 GitHub 设备码授权（Device Flow, RFC 8628）注册 OAuth App 并配置 `VITE_SYNC_GITHUB_CLIENT_ID`。只需 Client ID，无需 client secret。

## 一、注册 OAuth App

1. 打开 [github.com/settings/developers](https://github.com/settings/developers) → **OAuth Apps** → **Register a new application**

2. 表单按下表填写：

| 配置项 | 填写内容 |
| --- | --- |
| Application name | `PicBed Switcher`（可自定义） |
| Homepage URL | `https://github.com/zyx3721/picsw-rust`（可自定义） |
| Authorization callback URL | `http://localhost:8080`（占位即可，Device Flow 不会真正回调） |

3. 勾选 **Enable Device Flow**（新版界面注册页直接提供该复选框；若注册页没有，创建后进入应用详情页勾选并保存）
4. `Expire user access tokens` 保持默认勾选即可（会下发 refresh token，不影响使用）
5. 点击 **Register application** 创建应用

## 二、获取 Client ID

进入应用详情页，复制页面顶部的 **Client ID**（`Iv1.` 或 `Iv23` 开头的字符串）。不要生成 Client Secret，本项目用不到。

> 若应用创建时漏勾了 Device Flow，在详情页找到 **Device authorization flow** 区块勾选 **Enable device authorization flow**，底部 **Update application** 保存即可。

## 三、配置 Client ID

同一个键名 `VITE_SYNC_GITHUB_CLIENT_ID` 支持三种配置途径，运行时读取优先级为：

**系统 / 用户环境变量 → 程序所在目录 `.env` → 应用数据目录 `.env` → 构建期注入值**

### 方式一：CI 构建注入（发布版默认，推荐）

1. 仓库 **Settings → Secrets and variables → Actions → New repository secret**
2. Name 填 `VITE_SYNC_GITHUB_CLIENT_ID`，Secret 填 Client ID
3. 推送 `v*` 标签触发 CI 后，值会经 Vite 注入前端产物、`option_env!` 编入 Rust 二进制

Client ID 本身是公开值，编译进产物没有安全风险；真正需要保密的 client secret 全程不涉及。

### 方式二：.env 文件（本地测试推荐）

复制仓库根目录的 `.env.example` 为 `.env`，填入实际值：

```dotenv
VITE_SYNC_GITHUB_CLIENT_ID=Iv23liKaIYlHbYDB9dZ5
```

放置位置二选一（运行时按此顺序查找）：

- **程序所在目录**：`picbed-switcher.exe` 同目录下放 `.env`，适合便携版就地配置
- **应用数据目录**：Windows 为 `%APPDATA%\com.picsw.desktop\.env`

本地开发（`npm run dev` / `npm run tauri dev`）时在仓库根目录放 `.env`，Vite 会自动读取同名前缀变量注入前端。

### 方式三：系统 / 用户环境变量

设置名为 `VITE_SYNC_GITHUB_CLIENT_ID` 的用户或系统环境变量，值为 Client ID，重启应用后生效。

## 四、验证是否生效

打开「云同步」页签 → 点击「连接 GitHub」：

- **配置成功**：对话框内直接出现设备码（形如 `XXXX-XXXX`）与「打开 github.com/login/device」按钮，按提示完成授权即自动连接
- **未配置**：对话框提示「未检测到 GitHub OAuth App 的 client_id…」，此时可改用个人访问令牌（PAT）方式连接

## 五、常见问题

**Device Flow 授权报 `device_flow_disabled`？**
OAuth App 未启用 Device Flow，回到应用详情页勾选并保存。

**回调地址填什么？**
Device Flow 全程不发生页面跳转，`http://localhost:8080` 只是表单必填占位。

**改了配置不生效？**
环境变量方式需完全退出应用（含托盘）后重启；`.env` 方式确认文件名无后缀（不是 `.env.txt`）、键名拼写一致。
