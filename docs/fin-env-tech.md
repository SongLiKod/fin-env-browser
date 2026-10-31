\# FinEnv 指纹环境 技术设计文档

产品名称：FinEnv 指纹环境

仓库名称：fin-env-browser

文档版本：V4.1

基础框架：Tauri 2.0

文档用途：开发落地、架构评审、CI配置、维护参考



\## 1 整体架构说明

核心原则：\*\*Tauri 仅做管理控制台，被测网页在外部独立 Chromium/Firefox 子进程运行，禁止使用 Tauri WebView 加载待测页面\*\*

四层架构：

```

【前端 UI 层 React+TS】 <--Tauri IPC--> 【Rust 调度核心层】

↓

【CDP 控制器 \& 进程生命周期管理】

↓

【Chromium / Firefox 隔离进程集群（独立 profile）】

```

\- UI层：环境列表、指纹表单、代理配置、审计日志、主题设置、API密钥管理

\- Rust调度核心：IPC权限校验、进程启停监控、加密存储、资源限流、内置HTTP API服务

\- CDP控制器：维护WebSocket连接、注入指纹Hook、截图、录屏、页面脚本执行

\- 浏览器实例层：独立进程、独立用户目录、独立调试端口，和主面板解耦；面板崩溃不强制杀掉浏览器进程（可选）



\## 2 技术栈明细

\### 2.1 Tauri 2.0 主程序（管理面板）

\- 后端语言：Rust 1.80+

\- 异步运行时：tokio

\- IPC：Tauri Capability（白名单权限模型，严格控制前端可调用能力）

\- 本地数据库：rusqlite（SQLite）

\- 加密库：ring（AES256-GCM）

\- HTTP API服务：Axum（可选开启，独立端口）

\- 日志：tracing，结构化日志，输出文件+数据库审计表

\- 系统能力：托盘、全局快捷键、文件对话框、自动更新、系统通知

\- 移动端模板：提供 Pixel 8、Galaxy S24、Xiaomi 14、iPhone 15 Pro 模板，支持 `-landscape` 横屏变体；移动 UA、`navigator.platform`、Client Hints platform/model/mobile、触摸点和屏幕尺寸保持一致。屏幕方向通过 `screen.orientation` / `window.orientation` Hook 同步；地理定位通过 JS Hook 与 CDP `Emulation.setGeolocationOverride` 覆盖，并授予 geolocation 权限。移动端屏幕最小限制为 320×320。
\- 网络模拟：环境级 `network_profile` 支持 3G / 4G / 5G / 慢速 / 10% 丢包 / 离线；浏览器启动或恢复后通过 CDP `Network.emulateNetworkConditions` 应用，运行中修改会立即应用到当前受控标签页。`offline` 是标签页级网络模拟，不切断操作系统网络，也不会影响新开标签页或其他浏览器。\n\- 内置插件：插件是随应用编译的本地能力，不是任意远程代码；设置页维护全局 `enabled_plugins`，全局开启后环境级 `env_plugins` 覆盖开关生效，全局关闭时环境详情隐藏插件页签。插件启用后由 `BrowserManager` 的 CDP 事件循环接收网络事件。\n\- 登录凭据：登录成功后可选保存用户名/密码，使用本机 CryptoManager AES 加密存入 settings meta，登录页启动时解密回填；修改密码时清除保存记录。\n\- 验证码聚合：注入脚本仅在 `document.visibilityState === 'visible'` 时上报；已处理记录可批量标记，清理时同时删除截图文件。首个插件「视频嗅探」按环境内存保存媒体 URL（最多 300 条），通过 `video_list` / `video_clear` / `video_download` 操作；直接媒体使用环境代理、Cookie、Referer 流式下载。列表加载时还会扫描 `<video>/<audio>/<source>` 与 `PerformanceResourceTiming`，因此 `preload=none` 的页面无需先播放也能发现直接 MP4。浏览器级 CDP 会自动附着所有 page target，按 `visibilityState` / `hasFocus` 识别当前活动标签页；主 frame 导航、标签切换或新标签页打开后，列表按活动页面 URL 过滤并刷新，不再展示上一页结果。B 站 DASH 分片下载时会解析页面 BVID、请求 `x/web-interface/view` 与 `x/player/playurl`（html5 优先），将可用的 progressive MP4 直接下载为可播放文件。下载完成后检测 MP4 sample entry 中的 `encv`/`enca`，遇到 CENC/DRM 加密媒体会删除无效文件并提示使用「录制当前视频」；录制通过页面 `captureStream` + `MediaRecorder` 保存 H.264 MP4 到同一下载目录，并提供「下载目录」按钮；录制命令遍历所有 page target，选择实际存在视频轨的页面执行，避免新标签页/活动页判断导致误报“没有视频元素”。其他 HLS/DASH 地址保留复制能力。插件状态立即影响后续事件，但受控标签页之外的新标签页不会被捕获。\n\- 启动页动态更新：指纹/环境设置保存后重新生成对应 `start-pages/<envId>.html`；若该启动页正在当前受控标签页中打开，则自动刷新，展示最新地理定位、网络模拟、IP 与指纹摘要。\n\- 定位上下文：地理定位变化时使用内置离线时区边界数据推导 IANA 时区，并同步语言；该同步只覆盖时区/语言类指纹，IP 与 IP 位置仍由真实出口或代理决定，无法由页面 geolocation 伪造。
\- 桌面快捷方式：Windows 环境详情可生成 `.lnk`，目标为当前 FinEnv 可执行文件并带 `--env=<id>`；应用登录后读取启动参数，自动进入对应环境并打开浏览器。已运行时由单实例插件接管第二次启动，聚焦已有窗口并通过 `shortcut://env` 事件打开目标环境。
\- 环境身份：Windows 下将环境标识色应用到 Chromium 窗口边框，并通过点击穿透的分层窗口叠加 1-12px 可调粗细彩色边框；面板、运行栏与浏览器窗口使用同一颜色。任务栏按钮通过 ITaskbarList3 叠加同色圆点徽标，悬停/辅助功能描述包含环境名；每个环境可配置最多 4 个字符的标识文字，留空时显示纯色圆点。
\- 视觉层：界面使用极光渐变环境背景、玻璃质感卡片、深色渐变侧栏、渐变主按钮与统计卡片悬浮光效；所有颜色仍由主题主色和亮/暗模式驱动，不改变信息密度与操作路径。



\### 2.2 前端（Tauri 前端）

\- 语言：TypeScript

\- 框架：React 18

\- 构建工具：Vite

\- UI组件：shadcn/ui

\- 样式：TailwindCSS + CSS变量主题系统

\- 状态管理：Zustand

\- HTTP客户端：fetch，IPC调用封装层单独抽象

\- 主题系统：CSS Token驱动亮色/暗色/自定义主色



\### 2.3 浏览器内核 \& 控制层

\- 主内核：官方 Chromium（版本锁定，打包内置，也支持使用系统Chrome）

\- 备选内核：Firefox（Marionette协议）

\- 控制协议：CDP（Chrome DevTools Protocol）

\- 进程管理：Rust std::process + tokio::Command，托管子进程、监听退出信号、采集资源占用

\- 指纹注入方案：

&#x20; - V1：CDP Runtime.evaluate 注入JS Hook（快速落地）

&#x20; - V2：预编译内核补丁，内核级指纹篡改（高保真，迭代版本）

\- Chromium 关键启动参数：

```

\--user-data-dir={env\_profile\_path}

\--remote-debugging-port={random\_unique\_port}

\--proxy-server={proxy\_url}

\--no-first-run

\--no-default-browser-check

```

默认参数由 `DEFAULT_CHROMIUM_ARGS` 统一提供，设置页只读展示；全局和每个环境都可以在「浏览器启动参数」中追加每行一个自定义参数。启动时按“默认参数 → 全局参数 → 环境参数”合并，全局和环境取并集，同名开关以环境参数为准。profile、调试端口、代理、Hosts、扩展、语言等隔离参数禁止覆盖；窗口尺寸允许通过 `--window-size` 显式设置。窗口尺寸类参数存在冲突时环境优先：环境设置 `--window-size` 会移除全局的 `--start-maximized` / `--start-fullscreen` / `--kiosk`，环境设置最大化参数也会移除全局 `--window-size`。最终参数包含显式窗口尺寸或最大化参数时，不再追加默认 `--window-size`。





\### 2.4 SDK与自动化

\- JS SDK、Python SDK：封装HTTP API + CDP，提供给外部自动化脚本

\- 兼容Playwright：可复用Playwright连接到FinEnv分配的CDP端口



\### 2.5 辅助依赖

\- 指纹校验：请求BrowserLeaks/Pixelscan接口生成报告

\- 代理检测：tcp ping、http连通性检测、IP解析

\- 打包：tauri-cli，cross-rs 跨平台编译

\- CI：Gitea Actions / GitHub Actions



\## 3 仓库目录结构

```

fin-env-browser/

├── frontend/                     # Tauri 前端 React+Vite

│   ├── src/

│   │   ├── pages/                # 页面：环境列表、指纹配置、代理、审计、设置、主题

│   │   ├── components/           # 业务组件、表单、表格、弹窗

│   │   ├── theme/                # CSS 变量、主题 token、明暗主题定义

│   │   ├── ipc/                  # IPC 调用封装，向后端请求

│   │   ├── store/                # zustand 状态

│   │   └── utils/

│   └── vite.config.ts

├── src-tauri/                    # Tauri Rust 后端

│   ├── src/

│   │   ├── main.rs               # 入口

│   │   ├── process/              # 进程调度、监控、资源采集

│   │   ├── cdp/                  # CDP 连接管理、指纹注入、截图录屏

│   │   ├── db/                   # sqlite 表定义、迁移、CRUD、加密封装

│   │   ├── crypto/               # AES256 加密工具

│   │   ├── audit\_log.rs          # 审计日志写入

│   │   ├── api\_server/           # Axum REST API 服务

│   │   └── capability/           # Tauri 权限声明

│   └── tauri.conf.json

├── chromium-bin/                 # 内置锁定版本 Chromium（可选）

├── sdk/                          # JS SDK、Python SDK

├── docs/

│   ├── fin-env-requirement.md

│   ├── fin-env-tech.md

│   └── api-docs.md

├── scripts/                      # CI 打包、备份、环境迁移脚本

└── README.md

```



\## 4 主题自定义系统详细设计

\### 4.1 模式

\- light：亮色

\- dark：暗色

\- auto：跟随操作系统



\### 4.2 CSS Token（变量体系）

```css

:root {

&#x20; --color-primary: #165DFF;

&#x20; --color-primary-light: #E8F3FF;

&#x20; --color-primary-dark: #0E42C2;



&#x20; --bg-base: #ffffff;

&#x20; --bg-secondary: #f7f8fa;

&#x20; --bg-tertiary: #f2f3f5;



&#x20; --text-main: #1d2129;

&#x20; --text-weak: #86909c;

&#x20; --text-hint: #c9cdd4;



&#x20; --border: #e5e6eb;

&#x20; --border-strong: #dcdfe6;



&#x20; --danger: #f53f3f;

&#x20; --success: #00b42a;

&#x20; --warning: #ff7d00;



&#x20; --radius-sm: 4px;

&#x20; --radius-md: 8px;

&#x20; --radius-lg: 12px;

}



\[data-theme="dark"] {

&#x20; --bg-base: #17171a;

&#x20; --bg-secondary: #232324;

&#x20; --bg-tertiary: #2a2a2c;



&#x20; --text-main: #f2f3f5;

&#x20; --text-weak: #86909c;

&#x20; --text-hint: #5e6573;



&#x20; --border: #2e2e30;

&#x20; --border-strong: #3a3a3c;

}

```

前端提供取色器修改 `--color-primary`；修改后持久化存入数据库主题配置表。

支持导出主题 json，导入恢复。

布局密度变量：紧凑模式 / 宽松模式，控制间距大小。



> 

> 边界说明：主题 CSS 仅作用于 FinEnv 管理面板，不会注入到 Chromium 待测页面，不影响指纹。



\## 5 数据库核心表设计（SQLite）



1\. environments 环境主表：id、name、group\_id、status、profile\_dir、proxy\_id、fingerprint\_snapshot\_id、created\_at

2\. accounts 账号台账：id、env\_id、username、password\_enc、remark、owner、status

3\. fingerprint\_snapshots 指纹快照：id、env\_id、fingerprint\_json、create\_time

4\. proxies 代理池表

5\. audit\_logs 审计日志表（不可删，仅追加）

6\. theme\_config 主题配置表：mode、primary\_color、density、config\_json

7\. users 团队用户表（团队版）

8\. environment\_templates 环境模板表：id、name、description、业务字段、fingerprint\_json、标识色/宽度、group\_id、proxy\_mode（none/fixed/pool）、proxy\_id、extension\_ids、资源限制

9\. environment\_members 环境成员表：env\_id、user\_id、access\_level（view/operate/manage）、created\_at、updated\_at；非超级管理员只能访问被分配的环境



所有敏感字段：password\_enc、cookie 等，AES256 加密存储，密钥本地派生，不硬编码。



\## 6 API 设计（Axum HTTP 服务）



基础路径 `/api/v1`



\- POST /envs 创建环境

\- GET /envs 列表查询

\- POST /envs/{id}/open 打开环境

\- POST /envs/{id}/close 关闭环境

\- GET /envs/{id}/screenshot 截图

\- GET /envs/{id}/cookies 获取 Cookie

\- PUT /envs/{id}/fingerprint 更新指纹

\- POST /envs/{id}/snapshot 创建快照

\- POST /envs/{id}/restore-snapshot 恢复快照



\## 7 资源管控实现（Rust 层）



全局配置项持久化：



\- max\_concurrent\_browsers：最大同时打开浏览器数量

\- idle\_timeout\_min：闲置自动关闭时间

\- memory\_limit\_mb：单进程内存告警阈值

Rust 后台线程定时轮询所有子进程 PID，读取 CPU、内存；超过阈值触发告警 / 关闭，写入审计日志。



\## 8 安全设计



1\. Tauri Capability 白名单，前端不能随意调用文件、进程等系统能力。

2\. 敏感数据 AES256-GCM 加密落盘，密钥基于本机设备信息派生。

3\. 浏览器进程独立沙箱，和主程序权限隔离。

4\. 高危操作（批量导出账号密码）二次确认，强制审计日志记录。

5\. 云同步包整体加密，服务端无法解密明文。

6\. CDP 端口随机，不固定，避免端口扫描。



\## 9 打包与兼容性矩阵



\- Windows：Win10/Win11；自动检测 WebView2 Runtime，缺失引导安装；支持安装包 + 便携绿色版

\- MacOS：dmg，支持签名公证

\- Linux：AppImage / deb

\- CI 自动产物：安装包、checksum、版本文件



\## 10 风险清单 \& 降级方案



表格



| 风险 | 应对方案 | 降级方案 |

| --- | --- | --- |

| Rust 学习成本高 | 先做 V0 原型验证业务流程 | V0 阶段使用 Node 脚本调度 Chromium，跑通业务，V1 再迁移 Tauri |

| Chromium 版本升级，CDP 接口变更、指纹脚本失效 | 固定 Chromium 版本；环境绑定内核版本；快照备份 | 多内核版本管理，环境绑定指定内核 |

| Windows 老机器缺少 WebView2 | 打包附带引导安装程序 | 提示用户手动安装 WebView2 |

| Tauri IPC 权限配置失误 | 严格最小权限原则，每个能力单独声明 Capability | 代码评审，IPC 调用全部加日志 |

| 浏览器进程僵死 | 定时心跳检测，超时强制 kill 进程 | 面板提供一键强制回收全部浏览器按钮 |



\## 11 开发规范



1\. 所有 Rust 异步代码统一 tokio；日志统一 tracing

2\. 前端所有样式走 CSS Token，禁止硬编码颜色

3\. 所有数据库变更必须写迁移脚本

4\. 所有子进程操作、高危接口必须写入审计日志

5\. 禁止在前端存储任何明文敏感信息

6\. 非超级管理员访问环境时，必须同时通过全局角色权限和 `environment_members` 环境成员级别校验；列表查询按成员关系过滤，禁止只依赖前端隐藏。



\## 12 实时同步控制

\- 多 Tab 会话：通过浏览器级 CDP `Target.setDiscoverTargets` 监听 `targetCreated/targetDestroyed/targetInfoChanged`，使用 `Target.attachToTarget(flatten)` 为每个页面 Tab 建立独立会话；主控新建 Tab 后自动在参与环境配对或创建对应 Tab，切换时激活对应从属 Tab，关闭时同步关闭。自然由 `target=_blank` 打开的从属 Tab 会优先配对，未配对的重复 Tab 自动清理。

\- 主控绑定：`BrowserManager::broadcast_start` 先启动缺失环境；主控窗口通过 `Page.bringToFront` 置前，从属窗口使用 `Emulation.setFocusEmulationEnabled` 保证后台窗口仍可接收 CDP 输入，主控保持真实可见性用于活动 Tab 检测。

\- 输入捕获：主控每个页面 Tab 通过 `Runtime.addBinding` + `Page.addScriptToEvaluateOnNewDocument` 注入捕获脚本，监听可信的鼠标、滚轮、键盘、粘贴与 `compositionend` 事件，将结构化 JSON 发送给 Rust；脚本会删除全局 binding 引用，减少页面直接伪造广播的入口。

\- 转发：Rust 使用独立 mpsc worker 串行消费事件，通过 flat session 的 `Input.dispatchMouseEvent` / `Input.dispatchKeyEvent` / `Input.insertText` 发送到其他环境的对应 Tab；鼠标坐标按主控/目标视口尺寸缩放，滚轮按坐标寻找可滚动容器并执行滚动。

\- 活动 Tab：主控窗口每 500ms 采样各 Tab 的 `document.visibilityState` / `document.hasFocus()`，活动 Tab 变化时通过 `Target.activateTarget` 激活参与环境的对应 Tab；页面 URL 变化时通过 `Page.navigate` 同步导航。

\- 生命周期：`Page.loadEventFired` 时重新绑定主控并安装捕获脚本；暂停只丢弃后续事件；停止会移除脚本、binding 与焦点模拟。参与窗口关闭时自动停止会话，所有操作写入审计。

\- 限制：不同步标签栏拖拽/固定/分组等浏览器自身管理操作；跨域 iframe 内的点击不在捕获范围；恶意页面在极短时间内理论上可尝试伪造 binding 输入，仅建议用于受信测试站点。

