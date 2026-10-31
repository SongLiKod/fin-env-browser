# FinEnv 指纹环境

FinEnv 是面向研发测试团队的多账号独立指纹环境管理工具。每个测试账号绑定一套完全隔离的 Chromium 环境，持久保留登录态、Cookie、缓存与指纹配置，同时提供 REST API、JS / Python SDK 与 Playwright CDP 接入。

- 管理面板：Tauri 2.0 + React 18 + TypeScript + Vite + TailwindCSS + Zustand
- 调度核心：Rust + tokio + rusqlite（SQLite）+ Axum + ring（AES-256-GCM）
- 浏览器：独立 Chromium 进程，独立 `user-data-dir`，独立随机调试端口
- 安全：本地加密、追加式哈希链审计、Tauri Capability 白名单、高危操作二次确认

> 管理面板自身不加载任何被测业务页面；被测页面始终运行在外部 Chromium 子进程中。

## 功能

- 环境与账号台账：分组、项目、dev/test/pre/prod、角色、版本、用例 ID、标签、搜索筛选
- 指纹配置：UA、屏幕、时区、语言、Canvas、WebGL、Audio、字体、WebRTC、Client Hints、地理定位、插件；内置 Windows / macOS / Linux / Pixel 8 / Galaxy S24 / Xiaomi 14 / iPhone 15 Pro 模板，支持横竖屏与移动端指纹；定位变化时自动同步时区与语言
- 内置一致性校验与检测面板：CDP 探针逐项对比实际值，评分 + 修复建议
- 指纹快照：保存/一键还原全套配置
- 环境标识：每个环境可设标识色与 1-12px 粗细，列表/详情/运行栏/浏览器窗口边框统一；Windows 下浏览器窗口使用可调厚度、紧贴内容区的点击穿透彩色边框；运行中环境常驻栏显示名称、IP、PID
- 环境启动页：未指定测试地址时自动打开本地生成的环境信息页，展示指纹、代理、IP、账号/快照、实时浏览器信息与地址输入框
- 浏览器身份标牌：Windows 下在浏览器右下角显示环境名称，点击穿透、随窗口移动、悬停淡出，不遮挡 Tab 栏；任务栏按钮叠加环境标识色圆点，可为每个环境设置最多 4 个字符的标识文字
- 出口 IP：一键检测直连或代理出口 IP/国家/城市/时区，结果持久化并在列表与详情展示
- 代理池：HTTP/HTTPS/SOCKS5，TCP 检测、出口 IP/国家/时区、批量分配、时区语言对齐
- 进程隔离：独立 profile、随机调试端口、WebRTC/DNS 防泄漏参数、崩溃接管
- 网络模拟：每环境可选 3G / 4G / 5G / 高延迟 / 10% 丢包 / 离线，通过 CDP 作用于环境当前受控标签页；设置修改后立即应用，新开标签页不继承该模拟
- 桌面快捷方式：为单个环境生成 Windows 快捷方式，双击启动应用并在登录后自动打开该环境；应用已运行时聚焦已有窗口并切换环境
- 内置插件：设置中可全局启用/暂停；全局开启后可按环境单独开关，全局关闭时环境页隐藏插件；首个插件「视频嗅探」采集受控标签页的媒体响应，直接 MP4/WebM 可下载到系统下载目录，B 站 DASH 分片会自动解析并合并为 MP4；加密/DRM 视频可录制为可播放 MP4，视频页支持直接打开下载目录
- 验证码聚合：仅当前活动标签页触发自动采集，后台标签页不采集；支持全部标记处理、清空已处理记录并从硬盘删除截图
- 登录便利性：可选记住密码，登录成功后使用本机 AES 加密保存，下次打开自动填充
- 启动参数：设置页展示默认 Chromium 参数并支持全局追加 `--flag` / `--flag=value`；每个环境也可配置自己的启动参数，同名参数以环境配置为准；profile、调试端口、代理等隔离参数不可覆盖
- 资源管控：全局并发上限、内存告警/自动关闭、闲置回收、崩溃日志、可选自动重启
- 自动化：REST API、JS/Python SDK、Playwright CDP、前置脚本、截图/录屏/HAR
- 回调：截图、录屏、验证码事件可推送到通用 Webhook / 禅道 / Jira（JSON）
- 团队权限：超级管理员、测试负责人、测试工程师、只读查看者；细粒度权限矩阵
- 环境级权限：非超级管理员只能看到并访问被分配的环境；每个环境可分配团队成员并设置查看 / 操作 / 管理级别，创建者自动获得管理权限；环境列表/详情显示当前用户权限，无权限的操作按钮、页签和功能自动隐藏
- 审计：操作人、时间、环境、账号、项目/用例、哈希链、禁止修改、JSON/CSV 导出
- 辅助测试：独立 Hosts、独立 Mock（CDP Fetch）、验证码聚合、HAR；截图/录屏目录一键打开
- 扩展管理：Chrome 应用商店在线安装（CRX 下载/解压）、本地目录安装、全局扩展库、启用/停用、编辑、批量分配、一键应用到全部环境、打开目录与删除清理
- 环境模板：从环境保存模板，复用指纹、业务字段、标识色、代理策略与扩展；一次批量创建 1-50 个独立环境
- 多环境批量操作：对选中环境批量打开/关闭/重启、访问同一 URL、刷新、执行 JS、截图，有限并发并展示逐环境结果
- 同步控制：选中 2 个以上环境并指定主控窗口，主控页面中的鼠标点击/移动/滚轮、键盘输入、粘贴与输入法文字实时同步到其他窗口；新建/切换/关闭 Tab 与页面导航同步；支持暂停/恢复/停止与顶部常驻状态栏
- 可折叠左侧栏：展开显示完整导航与用户操作，折叠为图标栏并持久化状态；运行中环境在列表中使用绿色运行图标、脉冲「运行中 PID」徽标与状态列高亮
- 存储：AES-256-GCM 字段加密、本地备份/恢复、可选零知识 WebDAV/目录加密同步
- 主题：亮色/暗色/跟随系统、主色取色器、紧凑/宽松、导入导出、刷新不丢失；默认界面包含极光渐变背景、玻璃质感卡片、深色侧栏、渐变主按钮与任务栏徽标

## 仓库结构

```
fin-env-browser/
├── frontend/          # React + Vite 管理面板
├── src-tauri/         # Rust 调度核心、数据库、CDP、Axum API
├── chromium-bin/      # 可选内置 Chromium（版本锁定）
├── sdk/               # JS/TS 与 Python SDK
├── docs/              # 需求、技术设计、API 文档
├── scripts/           # 图标、CI 校验、备份脚本
└── README.md
```

## 快速开始

### 环境要求

- Node.js 18+（推荐 20/22）
- Rust 1.80+（Windows 推荐 MSVC 工具链）
- Windows 10/11、macOS 10.15+ 或 Linux
- Windows 需要 WebView2 Runtime（安装包可引导下载）
- Chromium/Chrome。可将锁定版本放入 `chromium-bin/`，或在系统设置中指定路径

### 开发

```bash
cd frontend
npm install
npm run check          # tsc + vitest + vite build

cd ../src-tauri
cargo check
# 在仓库根目录启动桌面应用（需要 Tauri CLI）
cargo install tauri-cli --version "^2"
cargo tauri dev
```

`cargo tauri dev` 会自动执行 `frontend` 的 Vite dev server 并打开管理面板。

### 构建

```bash
npm run version:sync
npm --prefix frontend run build
cargo tauri build
```

产物位于 `src-tauri/target/release/bundle/`，包含 Windows NSIS/MSI、macOS dmg、Linux AppImage/deb。CI 工作流会附带 checksum 与 version 文件。

### 版本号

版本号唯一手工维护位置是根目录 `VERSION`。修改后执行 `npm run version:sync`，会同步应用、Tauri、Cargo、前端和 SDK 的版本；`npm run version:check` 用于校验。详见 [docs/versioning.md](docs/versioning.md)。

### 首次登录

首次运行自动创建本地超级管理员：

- 用户名：`admin`
- 密码：`FinEnv@2026`

可通过环境变量覆盖初始值：

```bash
FIN_ENV_ADMIN_USER=admin
FIN_ENV_ADMIN_PASSWORD='change-me-now'
```

登录后请立即在「系统设置 → 团队与权限」修改密码。默认账号会写入审计日志。

### 数据目录

默认：`%LOCALAPPDATA%\FinEnv`（Windows）/ `~/Library/Application Support/FinEnv`（macOS）/ `~/.local/share/FinEnv`（Linux）。

可用 `FIN_ENV_DATA_DIR` 覆盖，CI/测试推荐使用临时目录。

```
FinEnv/
├── finenv.sqlite3      # SQLite（敏感字段 AES-256-GCM）
├── master.salt         # 设备派生主密钥的随机盐（勿外泄）
├── master.key.enc      # 主密钥密文（设备密钥包裹，勿外泄）
├── profiles/           # 每环境独立 Chromium profile
├── screenshots/
├── recordings/
├── har/
├── reports/
├── backups/
└── logs/
```

## 核心实现说明

### 指纹注入

V1 使用 CDP `Emulation.setUserAgentOverride`、`setTimezoneOverride`、`setLocaleOverride` 与 `Page.addScriptToEvaluateOnNewDocument` 注入 JS Hook。Hook 覆盖：

- `navigator.userAgent/platform/vendor/languages/hardwareConcurrency/deviceMemory/plugins/mimeTypes/webdriver`
- `screen.*`、`window.devicePixelRatio`
- Canvas `getImageData/toDataURL` 确定性噪声
- WebGL `getParameter` / `WEBGL_debug_renderer_info`
- Audio `AnalyserNode` 噪声
- `FontFaceSet.check` 字体白名单
- WebRTC ICE candidate / SDP 过滤
- Client Hints `userAgentData.getHighEntropyValues`
- 验证码 DOM 监听（`Runtime.addBinding`）

同时使用启动参数关闭 QUIC、限制 WebRTC 非代理 UDP，并根据 Hosts 生成 `--host-resolver-rules`。

### 一致性校验

后端保存前强制执行，冲突直接拒绝：UA 与 Client Hints 版本/平台一致、合法 IANA 时区、屏幕尺寸合理、DPR/色深范围、WebGL 与 OS 匹配、WebRTC 公网 IP、插件与 Flash、字体数量等。前端实时展示错误与提示。

### 进程与资源

- 启动前检查全局并发上限，超限返回 `capacity_limit`
- 每 5 秒轮询进程树 CPU/内存；超过单环境阈值触发系统通知 + 面板告警
- `memoryEnforce` 开启时自动关闭超限环境
- 闲置超时按环境/全局配置回收
- 崩溃自动记录哈希链审计，可选自动重启（10 分钟最多 3 次）
- 面板重启后通过保存的 PID/端口/WS 接管仍运行的浏览器

### 安全

- `password_enc`、代理密码、同步密码、Cookie 缓存、指纹配置/报告全部 AES-256-GCM 加密
- 主密钥 HKDF-SHA256(设备标识 + 随机盐)，主密钥以设备密钥包裹保存在 `master.key.enc`，不硬编码
- 跨设备：口令加密备份/同步包内携带整体加密的主密钥，恢复后由目标设备密钥重新包裹；未加密备份仅限同机恢复
- HTTPS/WSS 远程传输使用 rustls
- Tauri Capability 仅开放窗口核心能力和文件对话框；无 Shell/文件系统/任意 HTTP 权限
- 高危导出（账号密码、Cookie、环境包）需要权限 + 二次确认 + 全局开关
- 审计表触发器禁止 UPDATE/DELETE，SHA-256 链可离线复核
- CDP 只监听 `127.0.0.1`，端口随机

### 备份与同步

- 本地备份：SQLite 一致性快照 + 元数据，可选 profile；支持口令加密
- 云同步：WebDAV 或本地/挂载目录；同步包整体 PBKDF2-SHA256(210k) + AES-256-GCM，服务端零知识
- 恢复前强制回收全部浏览器；恢复后退出登录

## API 与 SDK

- API 文档：[docs/api-docs.md](docs/api-docs.md)
- JS SDK：[sdk/js](sdk/js)（`@finenv/sdk`）
- Python SDK：[sdk/python](sdk/python)（`finenv-sdk`）

```bash
curl -H "Authorization: Bearer $FINENV_API_KEY" \
  http://127.0.0.1:17321/api/v1/envs
```

```ts
const client = FinEnvClient.fromEnv();
const runtime = await client.openEnvironment(env.id, { url: 'https://test.example.com' });
const { context } = await client.connectPlaywright(env.id);
```

## 校验与测试

```bash
npm --prefix frontend run check
cd sdk/js && npm install && npm run build
cd ../../src-tauri && cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

前端单元测试覆盖主题 token 派生/持久化、格式化、CSV/标签解析与权限标签。Rust 单元测试覆盖 AES-GCM 字段与加密包往返、错误口令拒绝、审计哈希链核心函数。

CI 配置见 `.github/workflows/build.yml` 与 `.gitea/workflows/ci.yml`。推送 `v*` tag 或在 Actions 中手动填写发布版本时，会构建 Windows/macOS/Linux 安装包并创建 GitHub Release。

## 验收对照

| 验收标准 | 实现 |
| --- | --- |
| 多环境登录不同账号，Cookie 不互通 | 独立 `user-data-dir` / profile / 进程 |
| 关闭程序重启，登录态保留 | profile 持久化；面板重启自动接管进程 |
| API 拉起/关闭/截图/读写 Cookie | Axum REST + SDK + 二次确认 |
| 自定义主题刷新不丢失 | 数据库 `theme_config` + localStorage 缓存 |
| 超资源上限不再新建 | `maxConcurrentBrowsers` 启动前检查 |
| 密码/Cookie 密文存储 | AES-256-GCM 字段/缓存 + 设备派生密钥 |

## 已知边界

- V1 仅 Chromium；Firefox/Marionette 在 V1.1 规划中
- V1 指纹为 CDP JS Hook，高保真内核补丁属于后续版本
- 直连环境仍使用系统 DNS；要获得 DNS 出口隔离请绑定 SOCKS5/HTTP 代理，Hosts 规则仅在本进程内生效
- WebRTC `public` 模式用于声明公网出口，实际候选地址仍取决于网络/代理；`disable`/`proxy` 模式会注入防泄漏策略
- 录屏为 PNG 帧序列 + manifest，可用 ffmpeg 合成视频
- 加密备份/同步包内会携带由口令整体保护的主密钥，可在新设备恢复；未加密本地备份仍依赖本机设备密钥
- 环境包/备份口令可迁移配置，但登录态建议同时迁移 profile 或使用 Cookie 缓存恢复
- 内置 Chromium 需自行放入 `chromium-bin/`，仓库不分发二进制
- 云同步需自备 WebDAV 服务；应用不内置代理售卖或云端浏览器集群
- 更新采用更新清单检查 + 下载地址交接；自动替换安装包需在发布流水线配置签名更新源

## 文档

- [需求规格说明书](docs/fin-env-requirement.md)
- [技术设计文档](docs/fin-env-tech.md)
- [REST API](docs/api-docs.md)
- [需求对照与校验记录](docs/verification.md)

## License

MIT
