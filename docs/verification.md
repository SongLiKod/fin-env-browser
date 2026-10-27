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
- 性能：进程调度 tokio；前端构建产物 ~455KB JS（gzip ~134KB）；启动目标 ≤2s 依赖本机环境。

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
- `node scripts/verify.mjs`：通过，0 提示（107 个 Tauri 命令注册、107 个前端 IPC 命令覆盖、30 条 API 路由、18 张表、Capability、主题 Token、需求/技术文档能力追踪）
- `cargo fmt --check`：通过
- `cargo check`：通过
- `cargo clippy --all-targets -- -D warnings`：通过（0 warning）
- 核心逻辑测试（独立 harness，无 Tauri 运行时依赖）：15/15 通过
  - AES-GCM 字段加密/解密、错误口令拒绝
  - 加密包往返、主密钥跨设备导入与本机包裹持久化
  - 指纹模板一致性、冲突拒绝、时区/WebRTC 校验、国家对齐、注入脚本内容
  - 环境/快照指纹落盘密文校验
  - 账号密码、Cookie 缓存密文落盘校验
  - 审计哈希链校验 + 禁止 UPDATE/DELETE
  - 数据库迁移幂等（001 + 002 + 003 + 004 环境身份迁移）
  - 环境标识色、粗细与出口 IP 字段落盘往返
  - 加密备份跨设备可移植（备份内携带主密钥）并完整恢复账号密码、代理密码、同步密钥与 Cookie 缓存
- 真实二进制端到端（debug 构建 + 临时数据目录 + 本机 Chromium/Edge）：
  - 健康检查、环境创建/列表、快照、审计链校验、删除二次确认、重启后数据持久化
  - 指纹探针：UA、`navigator.platform`、languages、时区、屏幕分辨率与配置一致，`navigator.webdriver === false`
  - 浏览器打开/导航/PNG 截图；面板重启后重新附着同一进程（PID 不变、保留当前 URL）
  - Cookie 写入/读取/加密缓存落盘，应用重启后自动恢复
  - 连续录制 4 帧并生成 manifest；HAR 1.2 文件导出
  - 浏览器关闭与进程回收
- `cargo build`：通过；重新构建后的 `fin-env-browser.exe` 以临时数据目录启动，`GET /api/v1/health` 返回 `{"app":"FinEnv","status":"ok","version":"1.0.0"}`。
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
  - 前端 `npm run check` 通过（20 用例 + 构建）；`cargo fmt`/`clippy` 通过；核心 harness 15/15 通过；`scripts/verify.mjs` 0 提示。

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
