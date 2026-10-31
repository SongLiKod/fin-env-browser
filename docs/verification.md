# 需求对照与校验记录

本文把 `docs/fin-env-requirement.md` / `docs/fin-env-tech.md` 的条目映射到实际代码，并记录可重复执行的校验方式。

## 1. 环境与账号台账（P0）

| 需求 | 实现位置 | 校验 |
| --- | --- | --- |
| 新建环境自动生成自洽指纹，支持手动修改 | `fingerprint::template`、`services::create_environment`、`FingerprintEditor` | `cargo test generated_templates_are_consistent`；UI 新建后进入指纹配置 |
| 分组、项目、测试环境、角色、版本、用例 ID、标签、筛选搜索 | `db/env.rs`、`Environments.tsx` | UI 筛选项；`EnvironmentFilter` SQL |
| 账号字段、密码加密、四种状态 | `db/account.rs`、`AccountsPanel.tsx` | `crypto` 单测；账号编辑/导出 |
| 批量导入/导出、克隆、批量删除、加密环境包 | `package.rs`、`EnvironmentDialogs.tsx` | 多选后批量操作；`.finenv` 加密包 |
| 指纹快照保存/还原 | `services::create_snapshot` / `restore_snapshot`、`SnapshotsPanel.tsx` | UI；运行中立即 apply |
| 账号批量导入 | CSV 解析 + `bulk_insert_accounts` + 加密账号包 | 账号页「批量导入 / 导入账号包」 |

## 2. 指纹配置（P0）

| 需求 | 实现位置 |
| --- | --- |
| UA、屏幕、时区、语言、Canvas、WebGL、Audio、字体、WebRTC、Client Hints、插件 | `Fingerprint`、`FingerprintEditor.tsx` |
| 参数冲突禁止 | `fingerprint::validate`；保存/新建/还原/导入均强制 |
| 指纹检测面板 | `fingerprint_report::detect`、`PROBE_SCRIPT`、`DetectionPanel.tsx` |
| 一键调用检测服务 | 新开 CDP 标签页访问 `detectionServiceUrl`，探针比对并评分 |

## 3. 代理管理（P0）

- 每环境绑定 HTTP/HTTPS/SOCKS5：`environments.proxy_id` + 启动参数 `--proxy-server`。
- 连通性 + 出口 IP/国家/时区：`proxy_check.rs`、`Proxies.tsx`。
- IP/时区对齐：`services::align_proxy` + `fingerprint::apply_country_locale`。
- 代理池批量分配：`proxy_assign` / `AssignEnvsDialog`。
- 代理密码 AES-256-GCM：`proxies.password_enc`。

## 4. 浏览器内核隔离（P0）

- 独立进程、user-data-dir、随机调试端口：`process::open`、`free_port`。
- WebRTC/DNS：`--webrtc-ip-handling-policy`、`--force-webrtc-ip-handling-policy`、`--disable-quic`、`--dns-prefetch-disable`、Hosts resolver rules。
- 扩展单个/批量安装：`extensions`、`env_extensions`、`ExtensionsPanel`。
- 面板重启自动接管：`restore_sessions` + `browser_sessions` 表。
- 内核版本锁定：环境级/全局 `chromiumPath` + `chromium-bin/`。

## 5. 自动化与 API（P0）

| 能力 | 位置 |
| --- | --- |
| 环境 CRUD | `api_server/mod.rs` `/envs` |
| 打开/关闭/截图/Cookie 读写/改指纹/切代理/加载快照 | `/open` `/close` `/screenshot` `/cookies` `/fingerprint` `/proxy` `/restore-snapshot` |
| JS / Python SDK | `sdk/js`、`sdk/python` |
| 自动访问 URL + 前置脚本 | `OpenOptions.url/preScript` |
| 截图/录屏/验证码回调 | `BrowserManager::callback`；禅道/Jira 类型 |
| CI 调用 | API + SDK + `.github/workflows` |

## 6. 存储与同步（P0）

- 密码、代理密码、同步密码、Cookie 缓存：AES-256-GCM 字段级加密。
- 指纹 JSON（环境/快照/检测报告）：`Db::encrypt_json` 加密落盘；读取时按前缀解密，兼容历史明文。
- 主密钥：HKDF-SHA256(设备标识 + 随机盐)，并以设备密钥包裹保存至 `master.key.enc`；口令加密包可跨设备携带主密钥。
- 本地备份/恢复：`backup.rs`，SQLite 一致性备份 + 可选 profile + 可选口令。
- 零知识云同步：PBKDF2-SHA256(210k) + AES-256-GCM 整体加密，WebDAV/目录。
- `maintenance::upgrade_backup` 在版本变化后首次启动时自动为全部环境生成升级前指纹快照，并创建数据库备份。

## 7. 资源管控（P1）

- 全局并发：启动前检查 + `capacity_limit`。
- 内存阈值/自动关闭：5 秒进程树采样、通知、审计。
- 闲置回收：环境/全局配置。
- 实时 PID/CPU/内存/状态：`browser://runtime` 事件 + 列表/详情。
- 崩溃日志 + 可选自动重启：`handle_exit`，10 分钟最多 3 次。
- 一键强制回收：托盘菜单 + 环境页按钮。

## 8. 团队权限与审计（P1）

- 四种角色 + 12 项细粒度权限：`capability/mod.rs`，Tauri 命令和 API 双重检查。
- 高危导出全局开关：`Settings.allow_sensitive_export`。
- 审计字段、哈希链、禁止 UPDATE/DELETE 触发器、JSON/CSV 导出：`db/audit.rs`、`Audit.tsx`。
- 用户管理与密码修改：`Settings` 团队页 + 侧栏修改密码。

## 9. 辅助测试（P1）

- 独立 Hosts：`hosts_entries` + `--host-resolver-rules`。
- 独立 Mock：`mock_rules` + CDP Fetch fulfill，运行中即时生效。
- HAR 导出：`HarState` + `browser_har_start/stop`。
- 验证码聚合：DOM 监听 + CDP binding + `captcha_events` + 面板截图预览。

## 10. 主题（P1）

- light/dark/auto、主色、密度、导出/导入、持久化：`theme/*`、`store/theme.ts`、`theme_config`。
- 主题 CSS 仅面板；指纹注入脚本与主题完全分离。

## 11. 非功能

- 安全：Capability 白名单（核心 + dialog），无 fs/shell/http 插件权限；CDP 仅 127.0.0.1。
- 稳定性：SQLite 迁移；升级前自动快照与备份；浏览器进程独立于面板。
- 兼容：Windows/macOS/Linux Tauri 配置与 CI 矩阵。
- 性能：进程调度 tokio；前端构建产物 ~507KB JS（gzip ~149KB）；启动目标 ≤2s 依赖本机环境。

## 12. 可重复校验命令

```bash
# 前端类型、单测、生产构建
npm --prefix frontend run check

# JS SDK 构建
cd sdk/js && npm install && npm run build

# Python 语法
python -m compileall -q sdk/python/finenv

# Rust 格式、静态检查、单测
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## 13. 本轮校验记录

- `npm --prefix frontend run check`：通过（`tsc --noEmit` 0 error；Vitest 4 个文件 / 20 用例；Vite 生产构建成功，产物 `frontend/dist`）
- `sdk/js npm test`：4/4 通过（URL/Header/JSON/错误映射）
- `sdk/python python -m unittest`：4/4 通过（查询构造/请求体/错误映射/环境变量）
- Python 3.12 `py_compile`：通过
- `node scripts/verify.mjs`：通过，0 提示（122 个 Tauri 命令注册、122 个前端 IPC 命令覆盖、38 条 API 路由、19 张表、Capability、主题 Token、需求/技术文档能力追踪）
- `cargo fmt --check`：通过
- `cargo check`：通过
- `cargo clippy --all-targets -- -D warnings`：通过（0 warning）
- 核心逻辑测试（独立 harness，无 Tauri 运行时依赖）：16/16 通过
  - AES-GCM 字段加密/解密、错误口令拒绝
  - 加密包往返、主密钥跨设备导入与本机包裹持久化
  - 指纹模板一致性、冲突拒绝、时区/WebRTC 校验、国家对齐、注入脚本内容
  - 环境/快照指纹落盘密文校验
  - 账号密码、Cookie 缓存密文落盘校验
  - 审计哈希链校验 + 禁止 UPDATE/DELETE
  - 数据库迁移幂等（001 + 002 + 003 + 004 + 005 环境身份/模板迁移）
  - 环境标识色、粗细与出口 IP 字段落盘往返
  - 环境模板 CRUD：指纹、标签、代理策略、扩展 ID、资源限制往返与删除
  - 加密备份跨设备可移植（备份内携带主密钥）并完整恢复账号密码、代理密码、同步密钥与 Cookie 缓存
- 真实二进制端到端（debug 构建 + 临时数据目录 + 本机 Chromium/Edge）：
  - 健康检查、环境创建/列表、快照、审计链校验、删除二次确认、重启后数据持久化
  - 指纹探针：UA、`navigator.platform`、languages、时区、屏幕分辨率与配置一致，`navigator.webdriver === false`
  - 浏览器打开/导航/PNG 截图；面板重启后重新附着同一进程（PID 不变、保留当前 URL）
  - Cookie 写入/读取/加密缓存落盘，应用重启后自动恢复
  - 连续录制 4 帧并生成 manifest；HAR 1.2 文件导出
  - 浏览器关闭与进程回收
- `cargo build`：通过；重新构建后的 `fin-env-browser.exe` 以临时数据目录启动，`GET /api/v1/health` 返回 `{"app":"FinEnv","status":"ok","version":"4.1.0"}`。
- Windows GNU 链接阶段会输出 `.rsrc merge failure: multiple non-default manifests` 警告（MinGW 默认清单与 Tauri 资源清单合并提示），构建退出码为 0，产物可正常启动；CI 使用 MSVC 工具链，不涉及该提示。
- 完整 `cargo test` 的 Tauri 测试二进制在当前 MinGW 沙箱启动阶段报 `STATUS_ENTRYPOINT_NOT_FOUND`（WebView2/工具链运行环境限制）；`cargo check`、`clippy --all-targets` 已完整编译所有测试代码。`.github/workflows/build.yml` 在 Ubuntu 上执行完整单元测试，并在 Windows/macOS/Linux 矩阵中构建安装包。
- 浏览器预览：登录页亮/暗主题渲染正常，无 console error。

## 14. 环境身份、标识色与出口 IP（本轮 UI 优化）

- **运行环境常驻栏**：`RunningEnvironments` 位于顶栏下方，实时列出所有运行中的环境，显示标识色、名称、项目、出口 IP、PID，并提供刷新 IP、进入详情、关闭浏览器操作；每 5 秒同步一次，面板重启或 API 打开的环境同样可见。
- **浏览器窗口身份**：启动参数新增 `--window-name=FinEnv · <环境名>`，Chromium 窗口名称/任务栏标题可区分环境；面板与浏览器窗口使用同一套标识色。Windows 下额外在浏览器右下角显示可点击穿透的环境悬浮标牌，随窗口移动，鼠标移入时淡出，避免遮挡 Tab 栏。
- **环境标识色**：`environments.border_color`（迁移 003）保存 6 位十六进制颜色；`environments.border_width`（迁移 004，1-12px，默认 3px）控制标识色色条粗细。未设置颜色时按环境 ID 生成稳定调色板色。列表行左侧色条、环境名色点、详情页左侧色条、运行栏、创建/编辑对话框色板、取色器与粗细滑杆均使用该配置。
- **Windows 窗口边框**：Windows 11 下通过 DWM `DWMWA_BORDER_COLOR` 将同一颜色应用到真实 Chromium 窗口边框；其他平台自动忽略，不影响功能。
- **出口 IP**：新增 `POST /envs/{id}/check-ip` 与 IPC `env_check_ip`。绑定代理时经代理检测，否则直连检测，出口 IP/国家/城市/时区/来源/检测时间写入环境并在列表、详情、运行栏展示；代理变更时自动清空旧 IP。
- **界面修复**：修正 `Select` 组件宽度类作用在 `<select>` 而非外层容器导致的筛选区纵向堆叠；筛选区改为单行自适应并在第二行放置批量操作、环境计数与清除/分组管理。
- **本轮 UI/接口验证**：
  - 通过 WebView2 CDP 实际截图确认：筛选行横向排布、运行环境栏、环境标识色、详情 IP 卡片、创建对话框色板均正常。
  - 直连出口检测返回 `220.197.77.126 / China / Beijing / Asia/Shanghai`，并持久化到环境。
  - 非法颜色 `"borderColor": "red"` 返回 400；合法 `#0D9488` 可 PATCH 成功。
  - Chromium 窗口标题为 `FinEnv · <环境名>`；窗口边缘像素为设置的 `#16A34A`；无“不支持的命令行标记”提示；`navigator.webdriver === false`。
  - 前端 `npm run check` 通过（20 用例 + 构建）；`cargo fmt`/`clippy` 通过；核心 harness 16/16 通过；`scripts/verify.mjs` 0 提示。

## 15. 内置环境启动页与产物目录

- **默认启动页**：未指定打开 URL、且系统默认启动地址为空或 `about:blank` 时，`process::open` 生成 `start-pages/<envId>.html` 并以 `file://` 打开；headless 仍使用 `about:blank`。
- **页面内容**：环境名称/项目/测试环境/角色/版本/用例/标签/备注、出口 IP/位置/来源、代理、指纹摘要、账号与快照数、运行 PID/端口/内存、启动地址，以及从 `navigator`/`WebGL` 读取的实时浏览器信息；支持亮/暗色、标识色与 1-12px 粗细、地址输入跳转、复制/刷新。
- **动态更新**：打开环境时写入 PID/端口等运行信息；IP 检测、环境信息更新、代理对齐后会重写启动页文件，点击“刷新信息”即可看到最新内容。
- **产物目录**：`open_artifact_dir` 命令（`screenshots`/`recordings`/`har`/`exports`/`backups`/`reports`）调用系统文件管理器打开应用数据目录；截图/录屏卡片新增“图片目录/视频目录”按钮，并写审计 `artifact.open_dir`。
- **验证**：
  - 实际打开环境后 `currentUrl` 为 `file:///.../start-pages/<envId>.html`；启动页标题、标识色 `#2563EB`、粗细 `6px` 正确。
  - 启动页显示实时 `navigator.webdriver === false`、UA、平台、语言、时区、屏幕、WebGL。
  - Windows 环境标牌显示在浏览器右下角（`FinEnv · 环境名`），不占用 Tab 栏，点击穿透并在鼠标悬停时淡出。
  - 调用 `env_check_ip` 后重写启动页，刷新页面显示 `220.197.77.126 / Beijing / China / Asia/Shanghai`。
  - `open_artifact_dir('screenshots')` 与 `open_artifact_dir('recordings')` 均成功打开目录。
  - 启动页 HTML 转义测试覆盖环境名与备注中的 `<script>` 注入。

## 16. 扩展在线安装与全局管理

- **在线安装**：`extension_install_online` 支持 Chrome 应用商店链接或 32 位扩展 ID；后端下载 CRX2/CRX3，跳过签名头并解压 ZIP 到 `extensions/store/<id>-<suffix>`，校验 `manifest.json` 后加入全局扩展库。
- **安全**：拒绝非 CRX/ZIP 数据、超过 80MB 的安装包、超过 120MB 的解压总量、超过 6000 个文件；使用 `enclosed_name` 防止 Zip Slip，跳过符号链接。
- **全局扩展库**：任何环境都可选择同一扩展；列表展示已分配环境数，支持启用/停用、编辑名称/描述、打开扩展目录、删除（在线下载目录一并清理）。
- **批量与全部应用**：支持批量勾选环境、全选；`extension_assign_all` 一键分配到全部环境。运行中的环境需重启浏览器生效。
- **品牌版 Chrome 回退**：Chrome 137+ 品牌版移除了 `--load-extension`；在线安装页提供「在浏览器中打开商店」，可在目标环境中通过 Chrome 原生「添加至 Chrome」安装。Chromium / Chrome for Testing 仍可直接加载解压扩展。
- **验证**：通过本地 CRX3 测试服务完整跑通下载 → 解析 → 解压 → 入库 → 分配到 1 个环境 → 应用到全部 2 个环境 → 停用/改描述 → 第二个环境读取 → 删除并确认扩展目录被清理；UI 截图确认在线安装对话框与扩展库布局。

## 17. 环境模板、批量创建与多环境操作

- **环境模板**：迁移 005 新增 `environment_templates` 表，保存名称/说明、业务字段、标签、备注、内核与资源限制、完整指纹 JSON、标识色/宽度、分组、代理策略（`none` / `fixed` / `pool`）、固定代理与扩展 ID 列表。
- **从环境保存**：环境详情页「保存为模板」调用 `template_create_from_env`，复制当前指纹、业务字段、标识色、分组、固定代理与已分配扩展；模板名称唯一。
- **模板管理**：环境页「模板」入口支持列表、编辑名称/说明/代理策略、删除（二次确认）；批量创建对话框支持 1-50 个、起始序号、名称前缀与命名预览。
- **批量创建**：`template_batch_create` 为每个环境生成独立 profile 和唯一名称；固定代理直接复用，代理池按启用代理顺序轮询；创建失败逐条记录，不中断整批；每个环境仍写入创建审计，并额外记录 `template.batch_create`。
- **多环境批量操作**：环境列表多选后打开「批量操作」，支持打开、关闭、重启、访问 URL、刷新、执行 JavaScript、截图；前端以最多 3 个并发执行，逐环境展示成功/失败与错误原因。新增 `browser_reload` IPC 命令。
- **REST API**：新增 `GET/POST /templates`、`PATCH/DELETE /templates/{id}`、`POST /templates/{id}/create`，与 IPC 共用服务层、权限与审计。
- **权限**：模板列表沿用 `ViewList`；从环境创建模板/批量创建需要 `CreateEnv`；编辑/删除模板需要 `EditEnv`；删除模板和删除环境一样要求确认。
- **验证**：
  - `template_list` / `template_create_from_env` / `template_update` / `template_delete` / `template_batch_create` 已注册，前端 IPC 覆盖全部命令。
  - 核心 harness 新增模板 CRUD 往返测试：指纹、标签、代理策略、扩展 ID、资源限制落盘与更新/删除均通过。
  - `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过。
  - 前端 `npm run check` 通过（类型检查、20 个单测、生产构建）。
  - 真实 API 验证：创建源环境 → 保存模板（验证指纹/标签/标识色复制）→ 改名与代理策略更新 → 批量创建 3 个环境 → 校验 `01/02/03` 命名、独立 profile 目录、指纹/项目/标签/标识色复制 → 搜索到 3 个环境 → 打开浏览器、导航、刷新、执行 JS、PNG 截图、关闭 → 删除模板需确认且删除后列表消失。
  - 代理池验证：创建 2 个代理并将模板切换为 `pool`，批量创建 4 个环境后按启用代理顺序轮询为 A/B/A/B，固定代理缺失时会拒绝批量创建。
  - UI 验证（WebView2 CDP 截图）：模板列表、批量创建表单与命名预览、创建结果、多选 2 个环境、批量操作面板、批量打开结果（2/2 成功，显示 PID/端口）、批量刷新（页面已刷新）、批量访问（2/2 成功，显示目标 URL）均正常；浏览器 console 无 error。

## 18. 持续同步控制（多环境实时广播）

- **入口**：环境列表多选至少 2 个环境后点击「同步控制」；选择主控窗口、可选统一打开地址、是否自动打开未运行环境。
- **后端**：`broadcast_start/status/pause/stop` 四个 IPC 命令与 `GET/POST /broadcast`、`POST /broadcast/pause`、`POST /broadcast/stop` REST 接口；状态包含主控、参与环境、暂停状态、累计转发次数和未加入原因。
- **输入链路**：主控每个页面 Tab 通过 CDP `Runtime.addBinding` + `Page.addScriptToEvaluateOnNewDocument` 注入捕获脚本；Rust 用 mpsc worker 串行转发到其他环境的对应 Tab：
  - 鼠标移动（40ms 节流）、左/中/右键按下/释放、滚轮（按坐标寻找可滚动容器并滚动）；
  - 键盘按键、可打印字符、粘贴文本、输入法 `compositionend` 文本；
  - 主控页面导航/刷新后由 `Page.loadEventFired` 重新绑定并安装脚本。
- **多 Tab 同步**：浏览器级 `Target.setDiscoverTargets` 监听新建/销毁/URL 变化，`Target.attachToTarget(flatten)` 为每个 Tab 建立会话；主控新建 Tab 后自动在参与环境配对自然打开的 Tab 或创建对应 Tab；活动 Tab 通过 `document.visibilityState/hasFocus` 采样后 `Target.activateTarget` 同步激活；关闭主控 Tab 时关闭从属对应 Tab；未配对的重复 Tab 自动清理。页面 URL 变化通过 `Page.navigate` 同步。
- **生命周期**：启动时自动打开缺失环境并统一导航；主控窗口置前，从属窗口开启焦点模拟保证后台可接收输入；暂停只丢弃后续事件；停止移除脚本/binding/焦点模拟，保留浏览器窗口；参与窗口关闭自动停止会话；开始/暂停/恢复/停止全部写审计。
- **UI**：同步对话框展示主控、参与窗口、转发次数、未加入错误与暂停/停止操作；关闭对话框后顶部常驻同步栏仍显示状态，可暂停/恢复/停止。
- **验证（真实 API + CDP）**：
  - 单页面回归：选中 2 个环境启动同步，主控点击按钮后主控/从属点击数均为 1；输入 `abc` 后两边输入框均为 `abc`。
  - 滚轮转发后从属 `scrollY=600`，反向滚回 0。
  - 暂停后主控点击数 2、从属仍为 1；恢复后主控 3、从属 2。
  - 主控导航到新 URL 后重新绑定成功，点击后主控 1、从属从 2 增至 3。
  - 新 Tab：主控点击 `window.open('/newtab')` 后，两个浏览器均出现对应新 Tab（且无多余空白 Tab）；在新 Tab 中点击按钮/输入 `tab`，从属新 Tab 点击数 1、输入值为 `tab`；活动 Tab 切换日志确认从属对应 Tab 被 `Target.activateTarget` 激活。
  - Ctrl+T 等价路径：浏览器级 `Target.createTarget('about:blank')` 后，参与环境自动创建空白 Tab；关闭主控 Tab 后从属对应 Tab 同步关闭。
  - 停止后主控点击继续增加、从属保持不变；`GET /broadcast` 返回 `active: false`。
  - UI WebView2 CDP 截图确认开启对话框、同步进行中面板、顶部状态栏、暂停/恢复/停止流程；console 无 error。
- **已知限制**：不同步标签栏拖拽/固定/分组等浏览器自身管理操作；跨域 iframe 内的点击不在捕获范围；仅建议用于受信测试站点。

## 19. 左侧栏折叠与运行环境标记

- **折叠/展开**：顶栏左侧新增折叠按钮；折叠后侧栏宽度从 232px 变为 64px，仅保留 Logo、导航图标、运行数量、头像与改密/退出按钮；展开恢复完整工作区、资源管控卡片、用户名/角色与版本信息。
- **持久化**：状态写入 `localStorage` 的 `finenv.sidebar.collapsed`，刷新或重开面板后保持上次状态；折叠导航项保留 `title`/`aria-label`，便于识别。
- **运行标记**：环境中运行中的行显示绿色 Monitor 图标、带脉冲圆点的「运行中 PID xxxx」徽标、绿色运行状态列与右上角运行计数；未运行环境保持普通状态徽标。
- **验证（WebView2 CDP）**：
  - 展开态截图确认完整侧栏与运行环境标记；折叠后侧栏实测宽度 64px，`data-collapsed=true` 且 `localStorage` 为 `1`；再次展开宽度 232px、状态恢复为 `0`。
  - 运行环境行 `innerText` 同时包含「运行中」与 `PID <pid>`，绿色标记颜色为 `rgb(0, 180, 42)`。
  - 浏览器 console 无 error；`npm --prefix frontend run check` 通过。

## 20. 环境级团队访问控制

- **数据模型**：迁移 `006_environment_members.sql` 新增 `environment_members` 表，记录环境、用户和访问级别；访问级别为 `view` / `operate` / `manage`。超级管理员不受成员表限制，创建环境时自动为创建者写入 `manage`。
- **有效权限**：最终权限同时校验全局角色能力与环境访问级别；非超级管理员只能看到已分配的环境，直接调用 `env_get`、`browser_open`、账号、指纹、代理、快照、Hosts/Mock、扩展、克隆、删除、导出和模板等命令时，后端按环境成员关系返回 `403`。
- **管理入口**：环境详情新增「访问成员」页签，管理员和拥有 `manage_env_members` 的测试负责人可添加、调整、移除成员；成员候选列表不包含超级管理员。团队页仍负责全局角色和能力配置。
- **API 说明**：REST API Key 是管理员级自动化凭证，不受团队成员的按环境 ACL 限制；团队成员访问受控资源应使用 FinEnv 面板会话。
- **验证**：测试工程师只看到被分配的环境 A，环境 B 的列表/详情/打开请求均被拒绝；管理员访问成员页签可正常分配；环境创建者自动获得 `manage`。`cargo fmt`、`cargo clippy --all-targets -- -D warnings` 与前端 typecheck/build 通过。

## 21. 浏览器窗口可调粗细边框

- **实现**：`process/overlay.rs` 为 Windows Chromium 窗口创建 `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE` 的置顶覆盖窗口，按环境 `border_width`（1-12px）绘制上/下/左/右四条实色边框；边框内部使用 magic color key 保持透明。覆盖窗口随浏览器移动/缩放刷新，窗口过小或最小化时隐藏。
- **颜色**：覆盖边框使用环境标识色；同时通过 `DwmSetWindowAttribute(DWMWA_BORDER_COLOR)` 设置 Windows 11 原生边框颜色。
- **UI**：新建/编辑环境时可选择标识色并设置 1-12px 粗细；列表、详情和运行栏同步展示该颜色。
- **验证**：使用 `borderColor=#16A34A`、`borderWidth=10` 的环境打开真实 Chromium 窗口，截图像素采样确认 10px 绿色边框，点击穿透正常。

## 22. 环境权限展示与前端功能隐藏

- **权限回传**：`EnvironmentView` 新增 `accessLevel`（view/operate/manage），由后端根据当前登录用户与 `environment_members` 计算；超级管理员为 manage。环境列表与详情显示「我的权限 · 查看/操作/管理」徽标。
- **按钮/页签隐藏**：
  - 环境列表：查看级别隐藏打开/关闭、截图、克隆、删除、批量操作、同步控制、分配代理、批量删除等操作；操作级别可打开/截图/克隆，管理级别才显示删除/分组/代理等管理操作。
  - 环境详情：查看级别隐藏打开环境、检测指纹、保存模板、克隆、删除；只保留概览与账号台账只读页签。
  - 操作级别显示打开/截图/检测/调试，但不显示指纹配置、快照、网络与 Mock、扩展、访问成员等管理页签。
  - 账号面板、概览编辑、快照、网络、扩展、检测、调试、运行栏关闭按钮均同时校验全局角色权限与环境访问级别；无权限时不渲染或禁用。
- **验证（WebView2 CDP）**：管理员创建 `viewer` 用户并分配某环境 `view` 级别；该用户登录后环境列表只有 1 个环境、显示「我的权限 · 查看」，无打开/删除/克隆/截图按钮；进入详情只显示概览与账号台账，无打开环境、克隆、保存模板、指纹配置、访问成员、调试页签。管理员界面仍显示完整「访问成员」管理入口。

## 23. 环境列表紧凑排版

- **行内信息压缩**：环境名称、运行图标、权限徽标合并到第一行；运行状态、测试环境、首个标签合并到第二行，标签超过 1 个显示 `+N`，不再换行堆叠。
- **运行标识**：运行中仅显示绿色脉冲「运行中」小徽标，PID、CPU、内存和当前 URL 保留在「运行状态」列，不再占用环境名称列宽度。
- **权限徽标**：由「我的权限 · 管理」改为短徽标「管理权限 / 操作权限 / 查看权限」，hover 显示「我的环境权限：…」。
- **列宽与密度**：环境列最小 260px，项目/指纹/网络/状态/操作列使用固定宽度；环境表单元格横向内边距 8px、纵向内边距 8px，避免表格横向滚动和行高膨胀。
- **验证（1440×900 / WebView2）**：5 条环境记录行高约 65px（原实现约 100px 以上），表格宽度 1140px、无横向溢出；运行中环境仍可一眼识别，PID 仍可在运行状态列查看。前端 build 通过。

## 24. 浏览器边框贴合内容区

- **问题**：Windows 的 `GetWindowRect` 包含 Chromium 外侧不可见的调整边框（100% DPI 时左/右/下各约 8px，顶部为 0），旧覆盖层按外框绘制，导致绿色边框与页面之间在左、右、下出现空隙。
- **修复**：`frame_sync` 改用 `GetClientRect` + `ClientToScreen` 获取浏览器真实内容区，并把覆盖窗口定位到内容区；边框仍向内绘制，因此左/右/下与顶部一样紧贴页面。覆盖层从 Chromium 可见窗口中按面积选择主窗口，避免误绑「要恢复页面吗？」等辅助弹窗。
- **验证（真实 Chromium / 100% DPI）**：主窗口外框 `(91,74)-(2011,1106)`，客户区 `(99,74)-(2003,1098)`；边框覆盖窗口实测 `(99,74)-(2003,1098)`，与客户区完全一致。截图像素采样确认左、右、下绿色边框与页面之间不再有 8px 空隙，10px 边框仍可点击穿透。

## 25. API 基础路径可浏览

- **问题**：设置页展示的基础路径 `/api/v1` 没有注册路由，浏览器直接访问返回 404，容易被误认为 API 服务不可用。
- **修复**：新增公开的 `GET /api/v1` 服务信息页，返回服务名、版本、认证方式、健康检查地址和常用接口；业务接口仍需 API Key。健康检查 `GET /api/v1/health` 保持公开。
- **验证**：`GET /api/v1` 返回 200 JSON；`GET /api/v1/health` 返回 200；`GET /api/v1/envs` 未带 Key 返回 401；未知路径仍返回 404。

## 26. 版本号单一来源

- **唯一来源**：根目录 `VERSION` 是唯一手工维护的版本号。
- **同步**：`scripts/sync-version.mjs` 同步根 `package.json`、前端 SDK/锁文件、Python SDK、`src-tauri/Cargo.toml`、`Cargo.lock`、`tauri.conf.json` 和文档当前版本；`npm run build`、Tauri 前置命令与 `scripts/build.ps1` 自动执行同步。
- **校验**：`npm run version:check`、`npm run verify` 与 CI 的 Version consistency 步骤检查所有副本；`src-tauri/build.rs` 在编译前校验 `VERSION` 与 `Cargo.toml`。
- **UI**：登录页、侧栏、设置页和 API 版本均读取运行时版本，不再硬编码 `V1.0`。
- **验证**：`node scripts/sync-version.mjs --check` 返回一致；Tauri、Cargo、前端与 SDK 版本均跟随 `VERSION`。

## 27. 浏览器视频声音

- **问题**：Chromium 启动参数包含全局 `--mute-audio`，所有环境里的视频和音频都被静音。
- **修复**：移除 `--mute-audio`，浏览器按页面和系统音量正常播放声音；环境隔离、指纹和代理参数不变。
- **验证**：重新构建后启动环境，打开带声音的视频页面可正常输出音频；`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 通过。

## 28. 可查看与追加的浏览器启动参数

- **默认参数**：`process/mod.rs` 的 `DEFAULT_CHROMIUM_ARGS` 是默认静态参数；`AppInfo.chromiumDefaultArgs` 返回给设置页展示，避免前端复制一份默认列表。
- **用户附加参数**：设置 → 系统集成 → 浏览器启动参数新增文本域，每行一个 `--flag` / `--flag=value`，保存到 `Settings.chromiumLaunchArgs`，启动环境时追加到默认参数之后；留空恢复仅使用默认参数。
- **安全边界**：附加参数不能覆盖 profile 目录、调试端口、调试来源、代理、Hosts、扩展、语言等隔离参数；窗口尺寸允许通过 `--window-size` 自定义。单参数最长 512 字符、最多 64 项，拒绝控制字符和非 `--` 开头参数。
- **验证**：设置页显示默认参数列表与附加参数编辑框；保存后 `settings_get` 返回数组；启动环境的命令行包含附加参数且不包含被禁止的覆盖参数。

## 29. 环境级启动参数覆盖

- **数据**：迁移 007 为 `environments` 和 `environment_templates` 增加 `chromium_launch_args` JSON 数组；环境新建、编辑、克隆、模板创建和模板批量创建均保留该字段。
- **UI**：新建环境对话框和概览编辑增加「环境启动参数」文本域；全局设置页仍维护全局附加参数，并提示环境同名参数优先。
- **合并顺序**：默认静态参数 → 全局附加参数 → 环境附加参数；同名开关先移除全局项再追加环境项，因此环境配置生效。环境级参数同样经过格式、数量和隔离参数校验。
- **验证**：全局设置 `--disable-gpu`，环境设置 `--disable-gpu=false` 与 `--autoplay-policy=no-user-gesture-required`，启动命令只保留环境值；删除环境参数后恢复全局值。

## 30. 启动参数并集与最大化

- **并集**：全局参数先加入合并结果；环境参数逐项追加，只有同名开关会移除全局值，因此环境没有设置的全局参数不会丢失。
- **最大化**：Chromium 在收到显式 `--window-size` 时会忽略 `--start-maximized`。最终参数包含 `--start-maximized` / `--start-fullscreen` / `--kiosk` 或显式 `--window-size` 时不再追加默认 `--window-size`。
- **窗口冲突**：环境设置 `--window-size` 会移除全局 `--start-maximized` / `--start-fullscreen` / `--kiosk`；环境设置最大化参数会移除全局 `--window-size`。
- **验证**：全局仅设置 `--start-maximized`、环境参数为空，启动命令包含 `--start-maximized` 且不包含 `--window-size`，真实 Chromium 窗口 `IsZoomed=true`。全局 `--start-maximized` + 环境 `--window-size=800,600` 时窗口不最大化且尺寸为 800×600。全局 `--test-global` + 环境 `--test-env` 同时出现在命令行，同名 `--disable-gpu` 仅保留环境值。

## 31. 任务栏环境图标标识

- **实现**：`process/overlay.rs` 在覆盖线程内初始化 COM，使用 `ITaskbarList3::SetOverlayIcon` 为主 Chromium 窗口的任务栏按钮设置 32×32 覆盖图标；图标为环境标识色圆点（白色描边、透明背景），描述使用环境名。
- **生命周期**：覆盖图标随任务栏按钮建立后设置；窗口销毁时释放 HICON 并反初始化 COM。任务栏图标加载失败时不影响浏览器启动。
- **验证（Windows 11）**：启动标识色为 `#16A34A` 的环境后，任务栏 Chrome 按钮右上角显示绿色圆点，与管理面板和窗口边框颜色一致；不同环境保留各自任务栏按钮。

## 32. 任务栏标识文字

- **数据**：迁移 008 为 `environments` / `environment_templates` 增加 `taskbar_badge_text`，默认空字符串表示使用颜色圆点；新建、编辑、克隆、模板与批量创建均保留。
- **UI**：新建环境和环境概览增加「任务栏标识文字」，最多 4 个字符；留空显示环境标识色圆点。
- **图标**：任务栏覆盖图标使用 GDI 在 32×32 透明圆底上绘制白色文字，颜色仍取环境标识色；任务栏描述包含文字和环境名。
- **验证（Windows 11）**：环境设置 `taskbarBadgeText=A1` 后，任务栏 Chrome 按钮右上角显示绿色圆点 + 白色 `A1`；环境关闭后图标资源释放，其他环境互不影响。

## 33. 高端视觉刷新

- **背景**：`app-shell` 使用主色/紫色/青色三层极光径向渐变，26 秒缓慢漂移；`prefers-reduced-motion` 时关闭动画。
- **侧栏**：深色渐变玻璃侧栏、发光 Logo、渐变选中态与左侧光条；亮色和暗色模式均保持高对比。
- **内容区**：卡片玻璃质感、顶部高光、统计卡片悬浮光晕与浮起效果、渐变主按钮、渐变页标题、玻璃状态胶囊、表格悬浮主色微光。
- **兼容**：仅使用 CSS 变量与 `color-mix()`，不改变组件结构、路由或权限逻辑；窄屏/紧凑密度仍可正常使用。
- **暗色 Token**：`primary-light` / `primary-dark` 在暗色模式下重新计算为深色 tint / 提亮色，修复徽标、统计图标、主题选中卡片和色板仍显示白底的问题。
- **验证（WebView2，1440×900）**：亮色/暗色截图确认侧栏、统计卡片、环境表格、顶部状态与主按钮层级清晰，表格行高与信息密度不变；暗色环境下「已启用」徽标、主色浅色色板、选中主题卡片均为深色背景。

## 34. Android 移动端指纹模板

- **模板**：新增 `android` 模板：Android 14 / Pixel 8 UA（含 `Mobile`）、`navigator.platform=Linux armv8l`、Client Hints `platform=Android`、`mobile=true`、`model=Pixel 8`、ARM64、5 个触摸点、412×915 / DPR 2.625、Android 字体与 ARM Mali WebGL。
- **校验**：移动端屏幕最小限制调整为 320×480；UA 含 Android 时按 Android 校验 platform/Client Hints；移动端缺少触摸点或设备型号给出提示。
- **UI**：新建环境「指纹模板」和指纹编辑器模板按钮增加「Android 14 · 移动端」，Client Hints 增加 Android platform 与 model 输入。
- **验证**：`fingerprint_template(android)` 返回移动端模板，`fingerprint_validate` 返回 `valid=true`、0 error；创建 Android 环境成功并可按移动端尺寸启动。

## 35. 窄窗口边框与任务栏标识

- **问题**：移动端窗口宽度小于旧的 640px 阈值，边框层隐藏；任务栏覆盖图标在边框尺寸检查之后才设置，因此也一起消失；右下角悬浮名称同样被隐藏。
- **修复**：任务栏覆盖图标改为在窗口尺寸检查前设置，独立于窄窗口；边框最小阈值降到 180×180，悬浮名称阈值降到 260×260，移动端窄窗口仍显示。
- **验证**：Android 环境设置 `--window-size=412,915`（Chrome 实际客户区约 500×907），紫色边框完整显示、右下角显示「FinEnv · Android 测试」名称、任务栏 Chrome 按钮显示环境色圆点；全局 `--start-maximized` 不再覆盖环境窗口尺寸。

## 36. 移动设备、方向与地理定位

- **设备模板**：Pixel 8 / Galaxy S24 / Xiaomi 14 / iPhone 15 Pro，均支持 `-landscape` 横屏模板；移动 UA、platform、Client Hints、model、触摸点、屏幕尺寸和 DPR 成套生成。
- **方向**：模板横屏时交换屏幕宽高；注入 `screen.orientation` / `window.orientation`，竖屏为 0、横屏为 90。
- **地理定位**：Fingerprint 新增 `geolocation`（纬度/经度/精度），模板默认上海坐标；注入 geolocation Hook，并通过 CDP 授予权限和 `Emulation.setGeolocationOverride`。
- **验证**：6 个移动模板 `fingerprint_validate` 均返回 0 error；打开竖屏模板 `screen.orientation=portrait-primary/0`，横屏模板为 `landscape-primary/90`；页面 geolocation 返回配置坐标。

## 37. 网络模拟与桌面快捷方式

- **网络模拟**：环境新增 `networkProfile`（none/3g/4g/5g/slow/lossy/offline），新建与概览编辑均可设置；打开/恢复浏览器后通过 CDP Network 模拟生效。offline 下页面 `navigator.onLine=false`。
- **快捷方式**：Windows 环境详情生成桌面 `.lnk`，目标为 FinEnv 可执行文件，参数 `--env=<id>`；登录后自动进入环境详情并打开浏览器。
- **验证**：3G/4G/5G/慢速/丢包/离线配置可保存并应用；offline 下当前受控标签页的 `fetch` 失败、导航显示 `ERR_INTERNET_DISCONNECTED`。快捷方式目标、参数和图标正确；带 `--env=` 启动应用并登录后自动跳到环境详情、启动浏览器；应用已运行时再次带 `--env=` 启动会退出第二实例，并通过单实例事件在已有窗口打开目标环境。

## 38. 启动页动态刷新

- 保存指纹中的地理定位后，启动页 HTML 立即重写；若启动页正在当前受控标签页打开，则自动刷新并显示新的经纬度。
- 概览中修改网络模拟后，启动页同步刷新；offline 配置保存后当前受控标签页的 HTTP(S) 请求被阻断。
- 启动页新增「地理定位」「网络模拟」摘要与实时 geolocation 展示；设置变更不再需要手动关闭再打开环境。
- 定位改到东京坐标后，时区自动变为 `Asia/Tokyo`、语言变为 `ja-JP, ja, en`；改回上海坐标恢复 `Asia/Shanghai`。海上坐标没有可映射的时区时保留用户当前时区配置。
- IP 位置仍是真实出口/代理出口，不会被 geolocation 修改；页面上的「IP 位置」和「地理定位」分开展示。

## 39. 内置插件与视频嗅探

- 设置 → 插件可全局启用/暂停「视频嗅探」；状态保存在 `Settings.enabled_plugins`，无需重启应用即可对运行中受控标签页的后续响应生效。
- 视频列表通过 CDP `Network.responseReceived` 识别 `video/*`、`audio/*`、HLS/DASH MIME 及常见扩展名；同一 URL 去重，单环境最多保留 300 条。
- 环境详情新增「视频嗅探」页签，每 2.5 秒刷新；直接 MP4/WebM 等可下载，文件保存到系统下载目录下的 `FinEnv/`，按环境代理/Cookie/Referer 请求。
- 验证：打开 `flower.mp4` 测试页后识别到 `video/mp4`，下载文件大小 1,128,375 字节；清空列表返回空数组；暂停插件后不再接收新媒体。
- B 站 DASH：桌面页面抓到 `.m4s` 分片后点击「合并下载」，后端通过 BVID/CID 调用 B 站 view/playurl 接口解析 progressive MP4；实测 `BV1BCnW6XEf9` 下载为 8,977,990 字节的 MP4 文件。
- 限制：blob: 媒体、手动新开的标签页和通用（非 B 站）HLS/DASH 分片流不在当前 MVP 的自动合并范围内；通用流地址提供复制能力。
