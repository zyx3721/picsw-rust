<div align="center">

<h1>PicBed Switcher Desktop</h1>

<p><b>图床转站助手 桌面版</b> — GitHub · Gitee · 腾讯云 COS · 阿里云 OSS · 七牛云 · 百度云 BOS · 华为云 OBS · 又拍云 · MinIO · EasyImage</p>

<p><b>简体中文</b> · <a href="README.en.md">English</a></p>

Markdown 文档里的图床地址，换图床、换仓库、换域名时要逐篇逐链接手改。  
PicBed Switcher Desktop 把这套批量转换能力装进桌面应用：Rust + Tauri 2 内核 + Vue 3 工作区 + SQLite。  
下载一个安装包就能用，不登录、不部署、不占端口，离线可用。  
文档、转换历史与图床密钥都落在你自己的磁盘上；密钥以 AES-256-GCM 加密存储。

<p>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?labelColor=1f2937" alt="MIT License"></a>
  <a href="https://tauri.app/"><img src="https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri&logoColor=white&labelColor=1f2937" alt="Tauri 2"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.80%2B-DEA584?logo=rust&logoColor=white&labelColor=1f2937" alt="Rust 1.80+"></a>
  <a href="https://vuejs.org/"><img src="https://img.shields.io/badge/Vue-3-4FC08D?logo=vuedotjs&logoColor=white&labelColor=1f2937" alt="Vue 3"></a>
  <a href="https://www.sqlite.org/"><img src="https://img.shields.io/badge/SQLite-3-003B57?logo=sqlite&logoColor=white&labelColor=1f2937" alt="SQLite"></a>
</p>

<p>
  <b><a href="#项目预览">项目预览</a></b> ·
  <a href="#它做什么">它做什么</a> ·
  <a href="#怎么工作">怎么工作</a> ·
  <a href="#技术栈">技术栈</a> ·
  <a href="#快速开始">快速开始</a> ·
  <a href="#构建与安装">构建与安装</a> ·
  <a href="#支持的图床">图床</a> ·
  <a href="#数据与安全">安全</a> ·
  <a href="#常见问题">常见问题</a> ·
  <a href="#项目结构">项目结构</a> ·
  <a href="#文档">文档</a>
</p>

</div>

---

## 项目预览

### 工作区

| 桌面版工作区（转换 / 本地上传 / 图床配置 / 历史记录） |
| :---: |
| ![home](.github/images/picsw-desktop-home.png) |

## 它做什么

- **多图床适配** — 支持 GitHub、Gitee、腾讯云 COS、阿里云 OSS、七牛云、百度云 BOS、华为云 OBS、又拍云、MinIO 与 EasyImage 十类图床，以及兼容通用上传接口的自建服务。
- **文档转换** — 上传 Markdown 文档后自动识别其中的图床地址与类型，从已配置的图床中选择目标，一键批量转换并生成新文档下载。
- **本地上传** — 识别 Markdown 中的本地图片引用（相对路径、绝对路径、`<img>` 标签），把图片上传到目标图床后自动替换为远程地址。
- **转换任务队列** — 批量转换与本地上传都会创建持久化任务，由后台 worker 消费、任务内多文档并发转换（并发度 3）；应用重启后排队中的任务自动恢复执行，进度随时可查。
- **文件命名格式** — 上传对象路径支持 `{y}`、`{m}`、`{d}`、`{hash}`、`{random}`、`{rand:N}`、`{timestamp}` 等变量组合，按日期目录、内容哈希或自定义模板命名。
- **图床配置管理** — 添加、编辑、删除、测试多种图床配置，支持设置默认图床；敏感信息加密存储，界面展示脱敏。
- **转换历史** — 记录每次转换的源/目标图床、状态与图片数量，支持批量删除、搜索与时间范围筛选，详情中可查看每张图片的替换明细并下载转换结果。
- **图片下载** — 上传含网络图片的 Markdown，自动识别全部远程图片地址并标注图床类型，一键批量下载到自定义目录；同名文件自动加序号，不覆盖已有文件。
- **云同步** — 连接 GitHub 私有 Gist，把全部图床配置以同步密码加密备份上云：变更 3 秒后自动上传、随时手动同步，修订历史可浏览预览并一键恢复；换设备连接后输入同一密码解锁即可对齐，云端只存密文（零知识）。
- **关于页** — 查看应用版本与项目 GitHub 地址，一键直达仓库主页。
- **单实例运行** — 应用已打开时重复启动自动静默跳过，不弹窗、不聚焦。
- **GitHub 加速** — 检测到 GitHub 图片时，可选用 gh-proxy 类代理下载，提升批量转换成功率。

**它不是**图床本身，也不是图片压缩或 CDN 管理工具：PicBed Switcher Desktop 管的是「文档里图片地址的批量迁移与替换」，不代理图片流量，不缓存图片，不修改原图。

同一个换图床场景，放在两种做法里大致是这样：

| 现场 | 手工替换 | 用 PicBed Switcher Desktop |
| --- | --- | --- |
| 换图床 | 逐篇文档逐个链接手改 | 选好目标图床，批量转换一次完成 |
| 换仓库/换域名 | 查找替换容易漏、易错 | 自动识别图床类型与地址，精准替换 |
| 本地图片 | 先手动上传再逐个改链接 | 本地上传后自动替换为远程地址 |
| 转换进度 | 改到一半不知道到哪了 | 任务队列持久化，重启后继续跑 |
| 转换记录 | 无据可查 | 历史记录保留每次替换明细 |
| 图床密钥 | 散落在各处脚本里 | AES-256-GCM 加密存储在本地数据库 |

## 怎么工作

```text
  ┌────────────────────────────────────────┐
│  Tauri 窗口（系统 WebView）                │
│  Vue 3 工作区：转换 · 本地上传 · 配置 · 历史 │
└────────────────┬───────────────────────┘
                 │  Tauri IPC（invoke，不占网络端口）
                 ▼
┌────────────────────────────────────────┐
│  Rust 核心                              │
│  转换引擎 · 图床上传适配器 · AES-GCM 加密 │
│  任务队列（tokio mpsc + SQLite 持久化）   │
└────────────────┬───────────────────────┘
                 ▼
        SQLite（应用数据目录 picbed.db）
```

- **谁负责什么** — Vue 工作区负责选文档、选图床、展示任务进度与转换结果；文档解析、地址识别、图片上传与替换、任务调度全部由 Rust 核心执行。
- **数据放哪** — SQLite 是业务数据的事实来源，保存图床配置、转换任务与历史记录；图床密钥以 AES-256-GCM 加密落库，加密密钥派生方式与 Web 版一致。
- **任务队列** — 转换任务先落 SQLite 再进内存队列，由后台 worker 依次消费，任务内文档按并发度 3 同时转换；应用启动时会自动重新入队仍处于排队状态的任务。
- **对外暴露** — 前后端通过 Tauri IPC 通信，应用不监听任何 TCP 端口，只有访问图床 API 时才发起出站 HTTPS 请求。

## 技术栈

| 层 | 选型 |
| --- | --- |
| 应用框架 | [Tauri 2](https://tauri.app/)（Rust 内核 + 系统 WebView） |
| 后端语言 | Rust 1.80+（tokio 异步运行时） |
| 数据库 | SQLite（[rusqlite](https://github.com/rusqlite/rusqlite)，WAL 模式） |
| 图床上传 | [reqwest](https://github.com/seanmonstar/reqwest) + 自实现 S3 SigV4 签名（S3 兼容） |
| 加密 | AES-256-GCM（[RustCrypto](https://github.com/RustCrypto)） |
| 前端框架 | Vue 3 + TypeScript |
| 构建工具 | [Vite](https://vite.dev/) |
| 图标 | Lucide |
| 安装包 | NSIS（Windows）、dmg（macOS）、deb 与 rpm（Linux） |

## 快速开始

本地开发需要 **Rust 1.80+** 与 **Node.js 22+**；Windows 自带 WebView2，Linux 需先安装 WebKitGTK 依赖。

```bash
git clone https://github.com/zyx3721/picsw-rust.git
cd picsw-rust
npm install
npm run tauri dev
```

首次运行会编译 Rust（数分钟），完成后自动打开应用窗口，修改前端与 Rust 代码均支持热重载。

Linux 开发环境先装系统依赖：

```bash
sudo apt-get install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

验证与测试：

```bash
npm run type-check           # 前端类型检查
cd src-tauri && cargo test   # Rust 单元测试
```

## 构建与安装

### 方式一：下载 Release 安装包（推荐）

前往 [GitHub Releases](https://github.com/zyx3721/picsw-rust/releases) 页面，按自己的操作系统与 CPU 架构下载对应安装包：

| 你的机器 | 下载文件 |
| --- | --- |
| Windows x86_64 | `PicBed Switcher_<版本>_x64-setup.exe` |
| Windows ARM64 | `PicBed Switcher_<版本>_arm64-setup.exe` |
| macOS Intel 芯片 | `PicBed Switcher_<版本>_x64.dmg` |
| macOS Apple 芯片 | `PicBed Switcher_<版本>_aarch64.dmg` |
| Ubuntu / Debian 桌面 amd64 | `PicBed Switcher_<版本>_amd64.deb` |
| Ubuntu / Debian 桌面 arm64 | `PicBed Switcher_<版本>_arm64.deb` |
| Fedora / RHEL 桌面 amd64 | `picbed-switcher_<版本>_amd64.rpm` |
| Fedora / RHEL 桌面 arm64 | `picbed-switcher_<版本>_arm64.rpm` |
| Windows 便携版 amd64（免安装） | `picbed-switcher_<版本>_windows_amd64.zip` |
| Windows 便携版 arm64（免安装） | `picbed-switcher_<版本>_windows_arm64.zip` |
| macOS 便携版 Intel（免安装） | `picbed-switcher_<版本>_macos_amd64.tar.gz` |
| macOS 便携版 Apple 芯片（免安装） | `picbed-switcher_<版本>_macos_arm64.tar.gz` |
| Linux 便携版 amd64（免安装） | `picbed-switcher_<版本>_linux_amd64.tar.gz` |
| Linux 便携版 arm64（免安装） | `picbed-switcher_<版本>_linux_arm64.tar.gz` |

Windows 安装包支持 `/S` 静默安装；全部安装包由 CI 在对应架构环境下构建，推送 `v*` 标签时自动创建 Release 并附全套产物。

### 方式二：本地源码构建

```bash
npm run tauri build
```

产物位于 `src-tauri/target/release/bundle/`，Windows 为 NSIS 安装包，macOS 为 dmg，Linux 为 deb 与 rpm（rpm 打包需本机安装 `rpmbuild`）。

## 支持的图床

| 图床 | 所需配置 |
| --- | --- |
| GitHub | Personal Access Token、仓库信息 |
| Gitee | Private Token、仓库信息 |
| 腾讯云 COS | SecretId、SecretKey、存储桶 |
| 阿里云 OSS | AccessKeyId、AccessKeySecret、存储桶 |
| 七牛云 | AccessKey、SecretKey、存储桶 |
| 百度云 BOS | AccessKeyId、SecretAccessKey、存储桶、地域 |
| 华为云 OBS | AccessKeyId、SecretAccessKey、存储桶、地域 |
| 又拍云 | 服务名、操作员、密码、加速域名 |
| MinIO | Endpoint、AccessKey、SecretKey、存储桶（可选地域、SSL、公开访问域名） |
| EasyImage | API 地址、Token |
| 其他图床 | 兼容上传接口的 API 地址、Token |

除 EasyImage 外均支持自定义「文件命名格式」，可用变量：`{y}`、`{m}`、`{d}`、`{hash}`、`{origin}`、`{name}`、`{filename}`、`{ext}`、`{random}`、`{rand:N}`、`{timestamp}`；完整说明与格式示例见 [《使用手册》](docs/manual.md)。

## 数据与安全

```text
打开应用即用，无需登录
        +
图床 Token / 密钥 AES-256-GCM 加密落库，界面展示脱敏
        +
前后端 Tauri IPC 通信，不监听任何 TCP 端口
        +
全部数据保存在本机应用数据目录，可随取随备份
```

- **数据全在本地** — 配置、任务与历史记录保存在应用数据目录（Windows 为 `%APPDATA%\com.picsw.desktop\picbed.db`），不上传任何业务数据。
- **密钥加密存储** — 图床 Token / SecretKey 加密后落库，界面回显时敏感字段脱敏。
- **云同步零知识** — 云同步仅覆盖图床配置，以同步密码派生密钥（AES-256-GCM）加密后存入 GitHub 私有 Gist，云端只存密文；同步密码不落盘，GitHub 访问令牌存系统钥匙串（无钥匙串环境回退机器绑定加密文件）。连接支持设备码授权（配置见 [GitHub OAuth App 与 Device Flow 配置指南](docs/github-oauth-device-flow.md)）或个人访问令牌。
- **定期备份** — 关闭应用后备份应用数据目录中的 `picbed.db`（及其 `-wal`、`-shm` 文件）即可。

## 常见问题

**转换失败怎么办？**

按顺序检查：图床配置是否正确、Token/密钥是否有效、网络连接是否正常、图床 API 是否触发速率限制；转换历史详情中的错误摘要会给出具体原因。

**如何备份数据？**

关闭应用后，备份应用数据目录中的 `picbed.db`（及其 `-wal`、`-shm` 文件）；换机迁移时把备份文件放回新机器的同一目录即可。

**本地上传支持哪些图片引用？**

支持相对路径（`./images/a.png`、`images/a.png`）、Windows 绝对路径与 `<img src="...">` 标签；`http://`、`https://` 远程地址会保持不变，如需转换远程地址请使用批量转换。

**与 Web 版数据能互通吗？**

图床配置的加密方式与 Web 版一致，但存储位置不同（Web 版在 PostgreSQL，桌面版在 SQLite），暂不提供直接迁移工具；桌面版重新添加一次图床配置即可。

## 项目结构

```text
picsw-rust/
├── src-tauri/                Rust 后端与 Tauri 配置
│   ├── src/
│   │   ├── lib.rs            应用入口与命令注册
│   │   ├── main.rs           可执行文件入口
│   │   ├── commands/         Tauri 命令层（配置 / 转换 / 文件）
│   │   ├── engine.rs         转换引擎（远程转换与本地上传替换）
│   │   ├── uploader/         图床上传适配器（S3 SigV4 / REST / USS）
│   │   ├── task_queue.rs     任务队列 worker 与本地任务文件
│   │   ├── markdown.rs       图片提取 / 图床识别 / 地址替换
│   │   ├── crypto.rs         AES-256-GCM 配置加解密
│   │   ├── db.rs             SQLite 连接与建表
│   │   ├── models.rs         数据模型
│   │   ├── types_def.rs      图床类型元数据
│   ├── icons/                应用图标与安装器图像（源图 app-icon.png）
│   ├── tauri.conf.json       Tauri 打包与窗口配置
│   └── Cargo.toml
├── src/                      Vue 3 前端
│   ├── components/           工作区面板与对话框组件
│   ├── composables/          业务状态与逻辑
│   │   └── workspace/        转换、本地上传、配置等业务模块（request.ts 为 IPC 分发层）
│   ├── App.vue               应用根组件
│   ├── main.ts               前端入口
│   └── style.css             全局样式
├── public/                   静态资源（favicon）
├── .env.example              云同步 client_id 配置示例（复制为 .env 使用）
├── .github/                  GitHub Actions 工作流与预览图
├── LICENSE
├── README.md                 中文说明（本文件）
└── README.en.md              English
```

## 文档

| 先看这个 | 再往下 |
| --- | --- |
| [快速开始](#快速开始) | 本地起开发环境，跑起来改代码 |
| [构建与安装](#构建与安装) | 下载安装包或本地源码构建 |
| [使用手册](docs/manual.md) | 详细使用说明与文件命名格式 |
| [GitHub OAuth App 与 Device Flow 配置指南](docs/github-oauth-device-flow.md) | 云同步设备码授权的注册与配置 |
| [English README](README.en.md) | 同样的内容，英文版 |

## 版本历史

| 版本 | 发布日期 | 更新日志 |
| --- | --- | --- |
| v1.0.0 | 2026-09-29 | [verchanglog/v1.0.0.md](verchanglog/v1.0.0.md) |

各版本的构建产物与发布说明见 [GitHub Releases](https://github.com/zyx3721/picsw-rust/releases)。

## 致谢

感谢以下开源项目和技术社区的支持：

- [Tauri](https://tauri.app/) — 跨平台桌面应用框架
- [rusqlite](https://github.com/rusqlite/rusqlite) — Rust SQLite 绑定
- [reqwest](https://github.com/seanmonstar/reqwest) — Rust HTTP 客户端
- [RustCrypto](https://github.com/RustCrypto) — AES-GCM 等密码学实现
- [Vue](https://vuejs.org/) — 渐进式 JavaScript 框架
- [Vite](https://vite.dev/) — 快速前端构建工具
- [Lucide](https://lucide.dev/) — 简洁一致的开源图标库
- [picbed-switcher](https://github.com/zyx3721/picbed-switcher) — 本项目的 Web 版前身

也感谢所有为本项目贡献代码、提出建议和报告问题的开发者。

## 许可证

本项目采用 [MIT License](LICENSE) 开源协议，可自由使用、复制、修改、合并、发布、分发、再许可与销售，只需在所有副本或重要部分中保留版权声明与许可声明。

## 联系方式

- **Email**：416685476@qq.com
- **GitHub Issues**：[zyx3721/picsw-rust/issues](https://github.com/zyx3721/picsw-rust/issues)
- **项目主页**：[github.com/zyx3721/picsw-rust](https://github.com/zyx3721/picsw-rust)

---

**⭐ 如果这个项目对您有帮助，欢迎 Star 支持！**
