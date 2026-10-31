# FinEnv REST API 文档

基础路径：`http://127.0.0.1:17321/api/v1`（可在「系统设置 → API 服务」修改监听地址与端口）。

浏览器直接访问基础路径会返回 API 服务信息、认证方式和健康检查地址；业务接口需要 API Key。健康检查为 `GET /api/v1/health`，无需认证。

- 协议：HTTP/JSON（截图接口返回 `image/png`）
- 认证：`Authorization: Bearer finenv_xxx` 或 `X-API-Key: finenv_xxx`
- 高危确认：删除环境/代理等接口额外要求 `X-FinEnv-Confirm: true`
- 时区/时间：ISO 8601 UTC，例如 `2026-10-26T08:00:00.000Z`
- SDK：`sdk/js`（TypeScript）、`sdk/python`（Python 3.8+）
- 角色权限与面板一致：超级管理员 / 测试负责人 / 测试工程师 / 只读查看者

> 管理面板由 Tauri IPC 驱动；HTTP API 与面板共享同一套业务层、SQLite 数据库、进程管理和审计哈希链。

## 1. 认证与权限

在「系统设置 → API 服务 → API 密钥」创建密钥，选择角色。密钥仅在创建时显示一次，数据库保存 SHA-256 哈希与明文前缀。

| 权限 | super_admin | test_lead | test_engineer | viewer |
| --- | --- | --- | --- | --- |
| 查看列表 | ✅ | ✅ | ✅ | ✅ |
| 打开/关闭环境 | ✅ | ✅ | ✅ | — |
| 创建/编辑环境 | ✅ | ✅ | ✅ | — |
| 删除环境 | ✅ | — | — | — |
| 克隆环境 | ✅ | ✅ | ✅ | — |
| 修改指纹/代理 | ✅ | ✅ | ✅ | — |
| 导出 Cookie/账号 | ✅ | ✅ | — | — |
| 审计日志 | ✅ | ✅ | — | — |
| 系统设置 | ✅ | ✅ | — | — |
| 用户/API 密钥 | ✅ | — | — | — |
| 管理环境成员 | ✅ | ✅ | — | — |

所有出口 IP、账号密码、Cookie 等敏感读取均写入审计日志。管理员关闭「允许高危导出」后，Cookie GET/PUT 与账号导出返回 `403`。

> 环境级访问控制作用于 FinEnv 面板团队成员：除超级管理员外，用户只能看到并访问被分配的环境。每个环境可在「访问成员」中设置 `view` / `operate` / `manage` 级别。REST API 密钥目前是管理员级自动化凭证，不受团队成员环境分配限制。

## 2. 错误格式

```json
{
  "error": {
    "code": "confirmation_required",
    "message": "删除环境需要 X-FinEnv-Confirm: true"
  }
}
```

| HTTP | 常见 code | 场景 |
| --- | --- | --- |
| 400 | `bad_request` / `fingerprint_conflict` / `capacity_limit` | 参数冲突、达到并发上限 |
| 401 | `unauthorized` | 缺少/无效 API Key |
| 403 | `forbidden` | 角色权限不足、高危导出被关闭 |
| 404 | `not_found` | 环境/代理/快照不存在 |
| 409 | `conflict` | 同名环境、重复账号 |
| 500 | `internal_error` / `cdp_error` | 内部错误、CDP 连接失败 |

## 3. 公共接口

### GET /health

无需认证。

```json
{ "status": "ok", "app": "FinEnv", "version": "4.1.0", "time": "2026-10-26T08:00:00.000Z" }
```

### GET /stats

返回环境、账号、代理、运行实例、快照、审计数量与最近审计。

```json
{
  "envCount": 12,
  "accountCount": 36,
  "proxyCount": 4,
  "runningCount": 2,
  "snapshotCount": 8,
  "auditCount": 520,
  "totalMemoryMb": 936,
  "maxConcurrent": 30,
  "recentAudit": []
}
```

## 4. 环境

### GET /envs

查询参数（均可选，camelCase）：

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `search` | string | 名称、项目、标签、备注、用例 ID |
| `groupId` | string | 分组 ID |
| `status` | string | `active` / `disabled` / `archived` |
| `project` | string | 项目精确匹配 |
| `testEnv` | string | `dev` / `test` / `pre` / `prod` |
| `tag` | string | 标签 |
| `runningOnly` | bool | 仅运行中 |

```bash
curl -H "Authorization: Bearer $FINENV_API_KEY" \
  "http://127.0.0.1:17321/api/v1/envs?runningOnly=true"
```

返回 `EnvironmentView[]`，包含 `fingerprint`、`proxy`、`runtime`、`accountCount`、`snapshotCount`、`accessLevel`，以及环境标识、启动参数与最近一次出口 IP 检测字段：`borderColor`、`borderWidth`、`chromiumLaunchArgs`、`taskbarBadgeText`、`networkProfile`、`ipAddress`、`ipCountry`、`ipCountryCode`、`ipCity`、`ipTimezone`、`ipSource`、`ipCheckedAt`。`accessLevel` 是当前登录用户在该环境的权限（`view` / `operate` / `manage`）。

### POST /envs

最小请求：

```json
{ "name": "订单中心 - 测试 A", "templateOs": "win11" }
```

完整请求：

```json
{
  "name": "订单中心 - 测试 A",
  "groupId": null,
  "proxyId": "proxy-uuid",
  "project": "交易中台",
  "testEnv": "test",
  "accountRole": "运营",
  "version": "V2.8.1",
  "caseId": "CASE-1024",
  "tags": ["回归", "P0"],
  "notes": "仅用于 staging 回归",
  "borderColor": "#2563EB",
  "borderWidth": 4,
  "chromiumLaunchArgs": ["--autoplay-policy=no-user-gesture-required"],
  "taskbarBadgeText": "A1",
  "networkProfile": "4g",
  "templateOs": "win11",
  "autoRestart": false,
  "memoryLimitMb": 1500,
  "idleTimeoutMin": 30,
  "fingerprint": null
}
```

`templateOs`：`win11` / `win10` / `mac` / `linux` / `android` / `android-pixel8` / `android-galaxy-s24` / `android-xiaomi-14` / `iphone-15-pro`；移动端设备可加 `-landscape` 后缀切换横屏。不传 `fingerprint` 时自动生成自洽模板；传入时按内置规则校验，存在冲突返回 `400 fingerprint_conflict`。`borderColor` 为 6 位十六进制（如 `#2563EB`），留空则按环境 ID 自动生成标识色；`borderWidth` 为 1-12px，默认 3px，控制面板标识色色条和 Windows 浏览器窗口边框的粗细。`chromiumLaunchArgs` 是该环境追加的 Chromium 参数数组；与全局同名参数同时存在时环境值优先。`taskbarBadgeText` 是任务栏图标最多 4 个字符的标识文字，留空显示环境颜色圆点。`networkProfile` 取 `none` / `3g` / `4g` / `5g` / `slow` / `lossy` / `offline`。

成功：`201 Created` + `EnvironmentView`。

启动参数每项必须以 `--` 开头，最多 64 项、单项 512 字符；不能覆盖 profile、调试端口、调试来源、代理、Hosts、扩展、语言等隔离参数。环境参数与全局参数同名时，以环境参数为准；环境设置 `--window-size` 会覆盖全局最大化参数，环境设置最大化参数会覆盖全局 `--window-size`。任务栏标识文字最多 4 个字符，留空显示颜色圆点。

### GET /envs/{id}

返回 `EnvironmentView`。克隆、快照、指纹报告等子资源通过对应接口读取。

### POST /envs/{id}/check-ip

检测该环境当前的出口 IP。有绑定代理时经代理检测，否则走本机直连；结果写入环境字段并返回：

```json
{
  "envId": "env-uuid",
  "ok": true,
  "ip": "203.0.113.10",
  "country": "日本",
  "countryCode": "JP",
  "city": "Tokyo",
  "timezone": "Asia/Tokyo",
  "source": "proxy",
  "proxyId": "proxy-uuid",
  "latencyMs": 420,
  "message": "出口 IP 解析成功",
  "checkedAt": "2026-10-27T08:00:00.000Z"
}
```

### PATCH /envs/{id}

可更新字段均带默认值；`null` 表示清空代理/路径/阈值等：

```json
{
  "name": "订单中心 - 测试 A（复现）",
  "proxyId": null,
  "status": "active",
  "tags": ["回归"],
  "borderColor": "#16A34A",
  "borderWidth": 5,
  "chromiumLaunchArgs": ["--disable-gpu"],
  "taskbarBadgeText": "B2",
  "networkProfile": "3g",
  "autoRestart": true,
  "memoryLimitMb": null,
  "idleTimeoutMin": 0
}
```

### DELETE /envs/{id}

需要 `DeleteEnv` 权限与二次确认头：

```bash
curl -X DELETE -H "Authorization: Bearer $FINENV_API_KEY" \
  -H "X-FinEnv-Confirm: true" \
  "http://127.0.0.1:17321/api/v1/envs/$ENV_ID"
```

会先关闭浏览器进程，再删除环境/账号/快照/Hosts/Mock/profile；审计日志保留。

## 4.1 环境模板与批量创建

模板由已有环境保存而来，包含指纹、业务字段、标识色/宽度、分组、代理策略（直连 / 固定代理 / 代理池轮询）、扩展与资源限制。

### GET /templates

返回 `EnvironmentTemplate[]`。字段包括 `fingerprint`、`proxyMode`、`proxyId`、`extensionIds`、`chromiumLaunchArgs`、`taskbarBadgeText`、`networkProfile`、`borderColor`、`borderWidth`、`createdAt`、`updatedAt`。

### POST /templates

从已有环境创建模板：

```json
{
  "envId": "env-uuid",
  "name": "订单中心标准模板",
  "description": "运营账号 + 固定代理"
}
```

成功：`201 Created` + `EnvironmentTemplate`。同名模板返回 `409 conflict`。

### PATCH /templates/{id}

```json
{
  "name": "订单中心标准模板 V2",
  "description": "使用代理池轮询",
  "proxyMode": "pool",
  "proxyId": null
}
```

`proxyMode` 支持 `none` / `fixed` / `pool`；`fixed` 必须提供有效的 `proxyId`，`pool` 要求至少一个启用代理。

### POST /templates/{id}/create

按模板批量创建环境：

```json
{
  "templateId": "template-uuid",
  "count": 5,
  "namePrefix": "订单中心 CI",
  "startIndex": 1
}
```

`count` 为 1-50。名称形如 `订单中心 CI 01` … `订单中心 CI 05`，冲突时自动追加序号。成功返回：

```json
{
  "created": [],
  "errors": []
}
```

`created` 为已创建环境数组；某个环境创建失败不会中断整批，失败原因写入 `errors`。批量创建需要 `CreateEnv` 权限。

### DELETE /templates/{id}

需要 `EditEnv` 权限与 `X-FinEnv-Confirm: true`。仅删除模板，不影响已创建的环境。

## 5. 浏览器生命周期

### POST /envs/{id}/open

```json
{
  "url": "https://test.example.com/login",
  "preScript": "localStorage.setItem('token','ci-token')",
  "headless": false
}
```

- 达到 `maxConcurrentBrowsers` 返回 `400 capacity_limit`。
- 未传 `url` 且系统默认启动地址为空/`about:blank` 时，自动打开 FinEnv 本地环境启动页（`file://.../start-pages/<envId>.html`），展示环境、指纹、代理、IP 与实时浏览器信息；启动页不会发起外部请求。
- 返回 `BrowserRuntime`：`pid`、`debugPort`、`wsEndpoint`、`currentUrl`、`warnings` 等。
- 浏览器使用独立 `user-data-dir`、随机调试端口、独立 profile；关闭面板不会强制结束进程，重启面板后会自动接管。
- 使用代理时自动注入 `--disable-quic`、WebRTC 防泄漏参数；直连环境可配置独立 Hosts。

### POST /envs/{id}/close

幂等。关闭前将 Cookie 以 AES-256-GCM 密文缓存到数据库。

### GET /envs/{id}/status

```json
{ "running": true, "runtime": { "pid": 12480, "debugPort": 51234 } }
```

`runtime` 为 `null` 表示未运行。

### GET /envs/{id}/cdp

```json
{
  "envId": "env-uuid",
  "debugPort": 51234,
  "wsEndpoint": "ws://127.0.0.1:51234/devtools/page/...",
  "httpEndpoint": "http://127.0.0.1:51234",
  "running": true,
  "playwrightExample": "const browser = await chromium.connectOverCDP('http://127.0.0.1:51234');"
}
```

Playwright：

```ts
const browser = await chromium.connectOverCDP(cdp.httpEndpoint);
const context = browser.contexts()[0];
```

### POST /envs/{id}/navigate

```json
{ "url": "https://test.example.com/orders" }
```

仅允许 `http://`、`https://`、`about:blank`。

### POST /envs/{id}/reload

刷新当前页面，无请求体，返回 `{ "reloaded": true }`。对应面板批量操作中的「刷新」。

### POST /envs/{id}/execute

```json
{ "script": "document.title" }
```

返回 `{ "result": <Runtime.evaluate returnByValue> }`。脚本在页面主世界执行，支持异步 Promise。

### GET /broadcast

返回当前同步控制状态：

```json
{
  "active": true,
  "id": "session-uuid",
  "masterEnvId": "env-uuid",
  "envIds": ["env-uuid", "env-uuid-2"],
  "paused": false,
  "forwarded": 128,
  "startedAt": "2026-10-27T08:00:00.000Z",
  "message": "同步控制中",
  "errors": []
}
```

### POST /broadcast

开启持续同步控制。`envIds` 至少 2 个；`masterEnvId` 默认第一个；`url` 可选，开启前统一导航；`openMissing` 默认 `true`，自动打开未运行环境。

```json
{
  "envIds": ["env-uuid", "env-uuid-2"],
  "masterEnvId": "env-uuid",
  "url": "https://test.example.com/login",
  "openMissing": true
}
```

开启后，在主控浏览器页面中的鼠标点击/移动/滚轮、键盘输入、粘贴与中文输入法上屏文字会实时转发到其他参与环境。浏览器地址栏输入本身不可见，但页面 URL 变化会同步导航；主控新建、切换、关闭 Tab 时，参与环境的对应 Tab 会自动创建、激活或关闭。标签栏拖拽/固定/分组等浏览器自身管理操作不同步。

### POST /broadcast/pause

```json
{ "paused": true }
```

暂停后主控窗口仍可操作，但不再向其他窗口转发。改为 `false` 恢复。

### POST /broadcast/stop

```json
{ "reason": "测试完成" }
```

停止同步控制，浏览器窗口保持打开。参与窗口关闭时也会自动停止。

## 6. 截图 / 录屏 / HAR

### GET /envs/{id}/screenshot

默认返回 `image/png`：

```bash
curl -H "Authorization: Bearer $FINENV_API_KEY" \
  -o shot.png "http://127.0.0.1:17321/api/v1/envs/$ENV_ID/screenshot"
```

`?format=json` 返回：

```json
{
  "envId": "env-uuid",
  "path": "C:\\Users\\...\\FinEnv\\screenshots\\env.png",
  "dataUrl": "data:image/png;base64,...",
  "width": 1920,
  "height": 937,
  "capturedAt": "2026-10-26T08:00:00.000Z"
}
```

截图会触发回调推送（若配置）并写入审计。

### POST /envs/{id}/recording/start

```json
{ "intervalMs": 1000 }
```

按间隔采集页面 PNG 帧到 `recordings/<env>-<timestamp>/`，结束生成 `manifest.json`。

### POST /envs/{id}/recording/stop

返回：

```json
{
  "envId": "env-uuid",
  "directory": "...\\recordings\\env-20261026-080000",
  "frameCount": 42,
  "durationMs": 42000,
  "manifestPath": "...\\manifest.json",
  "startedAt": "...",
  "stoppedAt": "..."
}
```

### POST /envs/{id}/har

开始 Network 抓包。

### GET /envs/{id}/har

停止抓包并写出 HAR 1.2 文件：

```json
{ "path": "...\\har\\env-1730000000000.har", "envId": "env-uuid" }
```

## 7. Cookie

### GET /envs/{id}/cookies

需要 `ExportSensitive` + 高危导出开关。读取后同步更新加密缓存。

```json
{
  "envId": "env-uuid",
  "envName": "订单中心",
  "cookies": [
    {
      "name": "SESSION",
      "value": "abc...",
      "domain": ".example.com",
      "path": "/",
      "secure": true,
      "httpOnly": true,
      "session": false,
      "expires": 1790000000,
      "sameSite": "Lax"
    }
  ]
}
```

环境未运行时返回密文缓存（需要相同权限）。

### PUT /envs/{id}/cookies

```json
{ "cookies": [ { "name": "SESSION", "value": "new", "domain": ".example.com", "path": "/" } ] }
```

返回 `{ "updated": 1 }`。通过 CDP `Network.setCookies` 写入 profile。

## 8. 指纹与快照

### PUT /envs/{id}/fingerprint

请求体为完整 `Fingerprint`。示例（截取）：

```json
{
  "version": 1,
  "kernel": "chromium",
  "userAgent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
  "platform": "Win32",
  "vendor": "Google Inc.",
  "languages": ["zh-CN", "zh", "en"],
  "timezone": "Asia/Shanghai",
  "hardwareConcurrency": 8,
  "deviceMemory": 8,
  "doNotTrack": false,
  "colorScheme": "light",
  "touchPoints": 0,
  "screen": { "width": 1920, "height": 1080, "availWidth": 1920, "availHeight": 1032, "colorDepth": 24, "pixelRatio": 1 },
  "canvas": { "mode": "noise", "seed": 123456 },
  "webgl": { "vendor": "Google Inc. (Intel)", "renderer": "ANGLE (Intel, ...)", "seed": 42 },
  "audio": { "mode": "noise", "seed": 789 },
  "fonts": ["Arial", "Calibri", "Segoe UI", "Times New Roman"],
  "webrtc": { "mode": "disable", "publicIp": null },
  "clientHints": {
    "brands": [
      { "brand": "Chromium", "version": "131" },
      { "brand": "Google Chrome", "version": "131" },
      { "brand": "Not_A Brand", "version": "24" }
    ],
    "fullVersionList": [
      { "brand": "Chromium", "version": "131.0.6778.86" },
      { "brand": "Google Chrome", "version": "131.0.6778.86" },
      { "brand": "Not_A Brand", "version": "24.0.0.0" }
    ],
    "platform": "Windows",
    "platformVersion": "15.0.0",
    "architecture": "x86",
    "bitness": "64",
    "model": "",
    "mobile": false
  },
  "plugins": [
    {
      "name": "PDF Viewer",
      "filename": "internal-pdf-viewer",
      "description": "Portable Document Format",
      "mimeTypes": [{ "mimeType": "application/pdf", "description": "Portable Document Format", "suffixes": "pdf" }]
    }
  ]
}
```

一致性校验规则（节选）：

- UA 必须包含 Chrome 主版本，且与 Client Hints 主版本一致
- platform / clientHints.platform 必须与 UA 操作系统一致
- `colorDepth` 24/30/32，`devicePixelRatio` 0.5–4
- 时区必须为合法 IANA 时区
- WebGL Windows UA 不可使用 Metal
- WebRTC `public` 模式必须提供合法 IP
- 现代 Chrome 禁止 Flash 插件

成功返回更新后的 `EnvironmentView`。环境运行中会立即通过 CDP 应用新的 UA/时区/语言等。

### POST /envs/{id}/snapshot

```json
{ "name": "登录前基线" }
```

返回 `FingerprintSnapshot`。

### GET /envs/{id}/snapshots

按时间倒序返回快照数组。

### POST /envs/{id}/restore-snapshot

```json
{ "snapshotId": "snapshot-uuid" }
```

校验快照属于该环境；成功返回更新后的 `EnvironmentView`，运行中立即应用。

### POST /envs/{id}/detect

新开独立检测标签页访问系统设置中的检测服务，通过 CDP 探针生成逐项对比：

```json
{
  "id": "report-uuid",
  "envId": "env-uuid",
  "score": 92,
  "checks": [
    { "name": "User-Agent", "expected": "...", "actual": "...", "passed": true, "weight": 15, "hint": "..." }
  ],
  "observed": {},
  "configured": {},
  "screenshotPath": "...",
  "externalUrl": "https://browserleaks.com/javascript",
  "createdAt": "..."
}
```

## 9. 代理 / Hosts / Mock

### GET /proxies

返回代理池（不返回密码明文，只返回 `hasPassword`）。

### POST /proxies

```json
{
  "name": "上海出口",
  "protocol": "socks5",
  "host": "127.0.0.1",
  "port": 1080,
  "username": "user",
  "password": "secret",
  "enabled": true
}
```

`protocol`：`http` / `https` / `socks5`。密码立即 AES-256-GCM 加密。

### PATCH /proxies/{id}

同创建字段，均可选；`password` 留空/不传表示保持原密码。

### DELETE /proxies/{id}

需要 `X-FinEnv-Confirm: true`，并自动从绑定环境解绑。

### POST /proxies/{id}/check

TCP 连通性 + 通过代理请求出口 IP 服务：

```json
{
  "proxyId": "proxy-uuid",
  "ok": true,
  "latencyMs": 180,
  "exitIp": "203.0.113.10",
  "country": "US",
  "timezone": "America/New_York",
  "city": "New York",
  "message": "代理连通",
  "checkedAt": "..."
}
```

### POST /envs/{id}/proxy

```json
{ "proxyId": "proxy-uuid", "restart": false }
```

`proxyId: null` 切换直连。`restart: true` 时，若环境运行中会先关闭再以新代理启动。返回：

```json
{
  "updated": 1,
  "restarted": false,
  "restartRequired": true,
  "message": "代理已绑定，运行中的环境需重启后生效"
}
```

### POST /envs/{id}/align-proxy

根据代理出口国家自动更新指纹时区/语言，返回：

```json
{ "environment": { "...": "EnvironmentView" }, "check": { "...": "ProxyCheckResult" } }
```

### GET /envs/{id}/hosts

返回独立 Hosts 列表。

### PUT /envs/{id}/hosts

```json
{ "entries": [ { "host": "api.example.test", "ip": "10.0.0.12", "enabled": true } ] }
```

通过 Chromium `--host-resolver-rules` 注入；修改需重启环境。

### GET /envs/{id}/mocks
### PUT /envs/{id}/mocks

```json
{
  "rules": [
    {
      "pattern": "*://api.example.com/orders*",
      "method": "GET",
      "statusCode": 200,
      "headers": { "Content-Type": "application/json" },
      "body": "{\"code\":0,\"data\":[]}",
      "delayMs": 300,
      "enabled": true
    }
  ]
}
```

通过 CDP Fetch 域拦截，运行中的环境保存后立即生效；`*` 通配符按顺序匹配。

## 10. 审计

### GET /audit-logs

查询参数：`search`、`envId`、`action`、`actor`、`from`、`to`、`limit`（≤2000）、`offset`。

```json
[
  {
    "id": "audit-uuid",
    "actor": "admin",
    "action": "browser.open",
    "envId": "env-uuid",
    "account": null,
    "project": "交易中台",
    "caseId": "CASE-1024",
    "detail": "打开环境（PID 12480，调试端口 51234）",
    "createdAt": "...",
    "prevHash": "…",
    "hash": "…"
  }
]
```

### GET /audit-logs/verify

```json
{ "valid": true, "checked": 520, "brokenAt": null, "message": "已校验 520 条审计日志，哈希链完整" }
```

审计表由 SQLite 触发器禁止 UPDATE/DELETE，哈希链为 SHA-256(prevHash + 规范化字段)。

## 11. 回调推送

在「系统设置 → 系统集成」配置回调地址、类型（`generic` / `zentao` / `jira`）与可选 Bearer Token。以下事件在发生时会异步 POST JSON：

```json
{
  "type": "screenshot",
  "callbackType": "zentao",
  "timestamp": "2026-10-26T08:00:00.000Z",
  "envId": "env-uuid",
  "envName": "订单中心",
  "path": "...",
  "width": 1920,
  "height": 937,
  "capturedAt": "...",
  "dataUrl": "data:image/png;base64,..."
}
```

| type | 触发时机 | 主要字段 |
| --- | --- | --- |
| `screenshot` | 截图接口 / 面板截图 | `dataUrl`、`path`、宽高 |
| `recording` | 停止录屏 | `directory`、`frameCount`、`durationMs`、`manifest` |
| `captcha` | 自动/手动采集验证码 | `url`、`hint`、`screenshot`、`createdAt` |

回调失败只写 tracing 日志，不影响主操作；需要重试的团队可在自己的网关侧实现。禅道 / Jira 建议在网关把上述字段映射为对应 Issue 附件或评论。

## 12. SDK 示例

### JavaScript / TypeScript

```ts
import FinEnvClient from '@finenv/sdk';

const client = new FinEnvClient({
  baseUrl: 'http://127.0.0.1:17321/api/v1',
  apiKey: process.env.FINENV_API_KEY!,
});

const env = await client.createEnvironment({
  name: `CI-${Date.now()}`,
  project: '订单回归',
  testEnv: 'test',
  templateOs: 'win11',
});

const runtime = await client.openEnvironment(env.id, {
  url: 'https://test.example.com/login',
  preScript: "localStorage.setItem('ci', '1')",
});
console.log(runtime.pid, runtime.debugPort);

await client.saveScreenshot(env.id, 'login.png');
await client.closeEnvironment(env.id);
await client.deleteEnvironment(env.id);
```

### Python

```python
from finenv import FinEnvClient

with FinEnvClient.from_env() as client:
    env = client.create_environment(name="CI-订单回归", templateOs="win11")
    client.open_environment(env["id"], url="https://test.example.com/login")
    client.save_screenshot(env["id"], "login.png")
    print(client.detect_fingerprint(env["id"])["score"])
    client.close_environment(env["id"])
```

## 13. CI 集成

```yaml
- name: FinEnv smoke test
  env:
    FINENV_API_KEY: ${{ secrets.FINENV_API_KEY }}
    FINENV_BASE_URL: http://127.0.0.1:17321/api/v1
  run: |
    npm ci
    node --input-type=module -e "import('./sdk/js/dist/index.js').then(async ({default: FinEnvClient}) => { const c = FinEnvClient.fromEnv(); console.log(await c.health()); })"
```

CI 机器需要 FinEnv 桌面程序或后续独立 CLI 常驻；API 默认仅监听 `127.0.0.1`。跨机器访问时把监听地址改为 `0.0.0.0` 并通过防火墙限制来源，务必使用独立低权限 API Key。
