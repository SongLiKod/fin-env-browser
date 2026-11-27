# 代理池模块增强设计（订阅 / 批量导入 / 批量检测 / 分组标签）

- 文档版本：V2.0（P0/P1 已实现）
- 适用版本：FinEnv 4.3.x
- 状态：**已实现并接入前后端/HTTP API**；P2 项保留为后续增强
- 目标：在现有代理池基础上增加「订阅源、批量增加、批量检测、批量管理」，并补齐当前代理池在状态持久化、去重、分页和大数量场景下的短板。

---

## 0. 结论摘要

当前代理池是一个「单条 CRUD + 单条检测」的简单模型：

- 数据层只有 `proxies` 一张平表，没有来源、分组、标签、订阅、检测结论等字段。
- 前端「全部检测」是在 `Proxies.tsx` 里用 `for` 循环逐个调用 `proxy_check`，串行执行，没有并发、进度、取消和失败汇总。
- 检测失败只写回 `last_check_at/latency_ms`，不持久化失败原因；重载页面后只要历史上成功过，仍可能显示「可用」。
- 没有批量导入、批量启停、批量删除、批量改分组/标签。
- 没有订阅源，也没有自动刷新机制。

建议按「先补数据模型和批量能力，再加订阅，最后做分组/权重/API 完整化」三阶段实施。核心设计决策如下，确认后即可进入编码：

| 决策点 | 建议方案 | 原因 |
| --- | --- | --- |
| 批量检测执行方式 | 后台 Job + Tauri 事件进度 + 可轮询/可取消 | 避免长时间占用一次 invoke，支持关闭弹窗后继续执行 |
| 导入解析位置 | Rust 后端统一解析，前端只做预览展示 | 预览与提交使用同一套规则，避免前端/后端解析不一致 |
| 重复代理判断 | 新增 `endpoint_key`（协议+主机+端口+用户名），仅应用层去重；不合并历史数据，也不提供手动合并工具 | 非破坏性迁移；历史重复保留原样 |
| 订阅 URL/请求头 | 落库仍加密，接口/UI 可回显明文 | 按确认允许明文展示，同时避免数据库文件直接泄露 token |
| 订阅更新时缺失的代理 | 停用并标记，不删除；提供批量清理停用节点 | 保留环境绑定关系，可人工恢复或统一清理 |
| 订阅自动刷新 | 每个订阅独立开关，默认关闭；周期默认 6 小时可调；支持手动刷新 | 默认不产生后台网络请求，用户按需开启 |
| 批量并发 | 默认 8，范围 1-32，可在检测面板调整 | 单机 SQLite + 本地网络，8 路足够且稳定 |
| 分组/标签 | 放入 P0，成本低、对订阅管理收益大 | 订阅天然需要按来源/分组筛选 |
| 权重/模板按分组轮询 | 放入 P2 | 现有模板轮询可先保持「全部启用代理」 |
| HTTP API 批量检测 | 与桌面端一致，返回 `jobId` | 避免 HTTP 长连接超时，便于外部系统轮询 |

### 0.1 本轮已确认（2026-11-27）

1. **批量检测**：后台 Job + 事件进度 + 可取消。
2. **重复代理**：仅应用层去重；不合并历史数据，也不提供手动合并工具。
3. **订阅 URL / 请求头**：界面和 API 可以回显明文；落库仍复用现有 AES 加密，读取时解密返回，日志/审计不记录完整值。
4. **订阅缺失节点**：不删除，置为停用并标记；增加「批量删除停用状态节点」功能。
5. **订阅自动刷新**：默认关闭；周期可配置，默认 6 小时；始终支持手动刷新。

---

## 1. 现状分析

### 1.1 调用链

```
React 页面 Proxies.tsx
  └─ ipc.proxyList / proxyCreate / proxyCheck ...
       └─ Tauri command（commands/mod.rs）
            └─ services.rs（业务校验、审计、加密）
                 ├─ db/proxy.rs（SQLite 读写）
                 └─ proxy_check.rs（TCP + HTTP 出口检测）
                      └─ ipv4/ipv6 出口服务（ipwho.is / ip-api.com）

HTTP API（api_server/mod.rs）
  └─ 直接调用同一批 services 函数
```

关键文件：

| 层 | 文件 | 职责 |
| --- | --- | --- |
| 数据表 | `src-tauri/migrations/001_init.sql` | `proxies` 表、环境外键 |
| 数据访问 | `src-tauri/src/db/proxy.rs` | 代理 CRUD、检测结果写回、绑定计数 |
| 检测 | `src-tauri/src/proxy_check.rs` | TCP 8s + 代理 HTTP 12s、出口 IP/国家/城市/时区 |
| 业务 | `src-tauri/src/services.rs` | 创建/更新/删除/检测/分配/时区对齐 |
| IPC | `src-tauri/src/commands/mod.rs` | Tauri 命令与权限校验 |
| HTTP | `src-tauri/src/api_server/mod.rs` | REST 路由 |
| 前端 | `frontend/src/pages/Proxies.tsx` | 代理列表、编辑、分配、检测 |
| 类型 | `frontend/src/ipc/types.ts`、`client.ts` | 前后端契约 |

### 1.2 当前数据模型

`proxies` 现有字段：

```sql
id, name, protocol, host, port, username, password_enc,
enabled, last_check_at, latency_ms,
exit_ip, exit_country, exit_timezone,
created_at, updated_at
```

约束与关系：

- `protocol` 仅支持 `http` / `https` / `socks5`（`models::PROXY_PROTOCOLS`）。
- `password_enc` 使用 AES-256-GCM 加密，列表不返回明文。
- `environments.proxy_id`、`environment_templates.proxy_id` 外键指向 `proxies.id`，删除代理时 `ON DELETE SET NULL`。
- 没有唯一约束，允许重复添加同一 `host:port`。
- 没有分组、标签、来源、订阅、权重、检测结论字段。

### 1.3 现有能力

| 能力 | 现状 | 位置 |
| --- | --- | --- |
| 单条新增/编辑/删除 | 支持，删除需二次确认 | `services::create_proxy/update_proxy/delete_proxy` |
| 单条检测 | TCP + 通过代理请求出口服务 | `proxy_check::check_proxy` |
| 全部检测 | 前端串行循环，仅检测 `enabled` | `Proxies.tsx` 的 `checkAll` |
| 批量分配环境 | 支持勾选多个环境 | `AssignEnvsDialog` + `services::assign_proxy` |
| 出口时区对齐 | 支持 | `services::align_proxy` |
| 模板代理池 | 按启用代理列表轮询 | `services::batch_create_from_template` |
| HTTP API | 单条 CRUD + 单条检测 | `/proxies`、`/proxies/{id}/check` |
| 审计 | 每个单条操作一条 | `audit_log::actions::PROXY_*` |

### 1.4 关键缺口与问题

| # | 问题 | 影响 |
| --- | --- | --- |
| 1 | 没有订阅模型 | 只能手工维护，无法随供应商列表自动更新 |
| 2 | 只能单条新增 | 导入 100 条要调用 100 次或手工重复填写 |
| 3 | 「全部检测」串行 | 100 条 × 最坏 20s ≈ 30 分钟；无进度、无取消 |
| 4 | 检测失败原因不落库 | 刷新后看不到失败原因；旧 `exit_ip` 会让 UI 误判为可用 |
| 5 | 无去重 | 批量导入/重复刷新订阅会产生大量重复代理 |
| 6 | 无分组/标签/来源 | 订阅代理与手工代理混在一起，无法筛选和批量管理 |
| 7 | 无批量启停/删除/改分组 | 订阅失效后只能逐条处理 |
| 8 | 列表一次性全量返回 | 订阅上万条时前端卡顿、IPC 负载高 |
| 9 | 无分页/筛选 | 只能前端字符串过滤 |
| 10 | `Proxy::url()` 未处理 IPv6 | `[::1]:8080` 会拼成非法 URL |
| 11 | 检测结果国家字段语义混合 | 有时是国家码、有时是国家名，后续统计不稳定 |
| 12 | 批量检测若逐条审计 | 审计表膨胀；应汇总为一条 |

其中第 4 点是一个实际逻辑缺陷：前端判断状态使用

```ts
const ok = result?.ok ?? (proxy.exitIp ? true : null);
```

本次会话内的检测结果会覆盖，但重新加载后只要 `exit_ip` 非空就显示可用，即使最近一次检测失败。

---

## 2. 目标与非目标

### 2.1 目标

1. **订阅源**：支持创建、编辑、删除、启停、手动刷新、可选定时刷新（默认关闭）；支持常见文本/Base64 订阅，P1 支持 Clash YAML 中的 HTTP/HTTPS/SOCKS5 节点。
2. **批量增加**：支持粘贴文本、选择文件、从订阅同步；带预览、逐行错误提示、去重策略和事务提交。
3. **批量检测**：支持选中检测、按筛选条件检测、按订阅检测；并发执行、实时进度、取消、结果持久化、失败重试。
4. **批量管理**：批量启用/停用/删除、批量设置分组/标签；保留现有「单个代理分配到多个环境」的交互，不引入多对多绑定。
5. **在线发现**：从搜索引擎、GitHub、友情链接页面抓取公开代理节点，候选结果可勾选后直接加入代理池。
6. **可观测**：列表展示最近检测结论、失败原因、来源、分组、标签、绑定环境数。
7. **兼容**：现有单条 API、环境绑定、模板代理池、审计和加密行为不破坏。

### 2.2 非目标

- 不做代理售卖、代理测速排行、出口质量评分。
- 不支持 Shadowsocks / VMess / Trojan / Hysteria 等非 HTTP 代理协议；订阅中出现这些节点时跳过并计数。
- 不做跨设备订阅同步（现有云同步是整库/配置同步，另行考虑）。
- 不做代理自动切换/故障转移（环境仍固定绑定一个代理）。
- 不做复杂的权重调度，P2 再评估。

---

## 3. 总体设计

### 3.1 分层与职责

```
┌─────────────────────────────────────────────────────────────┐
│ React                                                       │
│  ProxiesPage                                                │
│   ├─ ProxyTable / ProxyToolbar / BulkActionBar              │
│   ├─ ProxyImportDialog（预览/提交）                          │
│   ├─ ProxyBatchCheckPanel（Job 进度/取消/重试）               │
│   └─ SubscriptionPanel / SubscriptionEditorDialog           │
└───────────────────────┬─────────────────────────────────────┘
                        │ Tauri invoke + events
┌───────────────────────▼─────────────────────────────────────┐
│ commands/mod.rs                                              │
│  proxy_query / proxy_import_preview / proxy_import_commit    │
│  proxy_batch_check / proxy_job_get / proxy_job_cancel        │
│  proxy_batch_update / proxy_batch_delete                     │
│  proxy_subscription_*                                        │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│ services.rs（业务编排）                                       │
│  ├─ proxy_import.rs（解析/规范化/去重键）                     │
│  ├─ proxy_subscription.rs（拉取/解析/diff/应用）              │
│  ├─ proxy_jobs.rs（内存 Job 注册表、并发、取消、进度）         │
│  ├─ proxy_check.rs（单条检测核心，可复用）                     │
│  └─ db/proxy.rs（分页查询、批量写入、订阅表）                  │
└───────────────────────┬─────────────────────────────────────┘
                        │
                    SQLite（单连接 + Mutex，事务批量写）
```

### 3.2 新增模块

| 模块 | 类型 | 说明 |
| --- | --- | --- |
| `src-tauri/src/proxy_import.rs` | 新文件 | 代理文本/Base64 解析（P0），JSON/YAML 解析（P1）、逐行错误、去重键 |
| `src-tauri/src/proxy_subscription.rs` | 新文件 | 订阅拉取、超时/大小限制、快照 diff、应用策略 |
| `src-tauri/src/proxy_discovery.rs` | 新文件 | 搜索引擎 / GitHub / 友情链接抓取、节点提取、候选内存库、选中添加 |
| `src-tauri/src/proxy_jobs.rs` | 新文件 | 批量检测 Job 注册表、并发控制、取消、进度事件 |
| `src-tauri/src/db/proxy.rs` | 扩展 | 新字段映射、分页筛选、批量插入/更新/检测写回、订阅 CRUD |
| `src-tauri/migrations/011_proxy_pool.sql` | 新文件 | 加字段、建订阅表、索引 |
| `frontend/src/pages/Proxies.tsx` | 重构 | 页面容器 + Tab + 数据加载 + 表格/导入/批量检测/订阅组件 |

### 3.3 三条核心流程

#### A. 批量导入

```
用户粘贴/选文件
  → proxy_import_preview(text, options)
      → 后端解析、规范化、检测批内重复/库内重复
      → 返回前 200 行预览 + 统计 + 错误行
  → 用户确认
  → proxy_import_commit(text, options)
      → 再次解析（防前端篡改）
      → 单事务批量插入/更新
      → 一条汇总审计
      → 返回 {inserted, updated, skipped, failed, errors}
  → 前端刷新列表；可选「立即检测新增代理」
```

#### B. 批量检测

```
用户选择代理/筛选条件
  → proxy_batch_check({ ids | filter, onlyEnabled, concurrency })
      → 创建 Job，立即返回 jobId
      → 后台 tokio 任务并发执行
          → 每完成一条更新 Job 内存状态
          → 节流 emit("proxy://batch", progress)
          → 每 25 条增量写回，结束/取消时写回剩余结果
      → 完成后写汇总审计
  → 前端监听事件更新进度；也可 proxy_job_get 轮询
  → 完成后使用 proxy_job_get 拉取完整 results
```

#### C. 订阅刷新

```
手动刷新 / 定时器到期
  → 读取订阅（解密 URL/Headers）
  → HTTP GET（超时、大小上限、UA、自定义头）
  → 解析为代理快照（去重、过滤不支持协议）
  → 与库内该订阅的代理做 diff
      ├─ 新增：插入
      ├─ 已存在：更新认证/协议（保留用户改过的名称与启停状态）
      ├─ 消失：停用并标记，不删除
      ├─ 恢复：若之前因缺失被自动停用，重新启用
      └─ 与手工代理冲突：默认跳过
  → 单事务应用
  → 更新订阅 last_refresh_*，一条汇总审计
  → emit("proxy://changed", { reason: "subscription_refresh" })
```

---

## 4. 数据模型设计

### 4.1 `proxies` 扩展字段

| 字段 | 类型 | 默认 | 说明 |
| --- | --- | --- | --- |
| `group_name` | TEXT | `''` | 分组名；不叫 `group`，避免 SQL 关键字 |
| `tags` | TEXT | `'[]'` | JSON 字符串数组 |
| `note` | TEXT | `''` | 备注 |
| `source` | TEXT | `'manual'` | `manual` / `import` / `subscription` / `discovery` |
| `subscription_id` | TEXT NULL | NULL | 来源订阅；订阅删除时 SET NULL |
| `endpoint_key` | TEXT | `''` | 规范化唯一键，应用层去重 |
| `weight` | INTEGER | `1` | 预留权重，P2 使用 |
| `last_check_ok` | INTEGER NULL | NULL | NULL 未检测 / 0 失败 / 1 成功 |
| `last_check_message` | TEXT NULL | NULL | 最近检测消息（失败原因会保留） |
| `last_check_city` | TEXT NULL | NULL | 最近出口城市 |
| `fail_count` | INTEGER | `0` | 连续失败次数；成功清零 |
| `disabled_reason` | TEXT | `''` | `''` / `manual` / `subscription_missing` |

> `last_check_ok` 是修复「旧 exit_ip 误判可用」的关键字段。UI 状态优先读它，其次才看旧出口信息。

### 4.2 新表 `proxy_subscriptions`

```sql
CREATE TABLE IF NOT EXISTS proxy_subscriptions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url_enc TEXT NOT NULL,
    headers_enc TEXT NOT NULL DEFAULT '',
    default_protocol TEXT NOT NULL DEFAULT 'http',
    group_name TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    auto_refresh INTEGER NOT NULL DEFAULT 0,
    refresh_interval_min INTEGER NOT NULL DEFAULT 360,
    last_refresh_at TEXT,
    last_refresh_status TEXT,
    last_refresh_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

字段说明：

- `url_enc`：AES-256-GCM 加密后的订阅 URL；**读取时解密，列表/编辑接口返回明文 `url`**（按确认）。
- `headers_enc`：加密的自定义请求头 JSON，可放 `Authorization`、`User-Agent`；保存前校验为对象，限制 20 个头、总大小 8 KB；接口返回明文 `headers`。
- `default_protocol`：纯 `host:port` 行使用的默认协议。
- `enabled`：停用后不参与手动/自动刷新（代理保留）。
- `auto_refresh` / `refresh_interval_min`：自动刷新开关与周期，默认关闭、周期 360 分钟，最小 5 分钟，可调。
- `last_refresh_status`：`ok` / `error`。

> 订阅缺节点不再使用 `missing_policy`，统一走「停用 + 标记」。代理侧通过 `disabled_reason` 区分用户手动停用与订阅缺失停用。

代理数量不冗余存储，列表用 `LEFT JOIN` 或子查询实时统计，避免同步失败导致计数漂移。

### 4.3 索引

```sql
CREATE INDEX IF NOT EXISTS idx_proxies_endpoint_key ON proxies(endpoint_key);
CREATE INDEX IF NOT EXISTS idx_proxies_subscription ON proxies(subscription_id);
CREATE INDEX IF NOT EXISTS idx_proxies_group ON proxies(group_name);
CREATE INDEX IF NOT EXISTS idx_proxies_source ON proxies(source);
CREATE INDEX IF NOT EXISTS idx_proxies_enabled ON proxies(enabled);
CREATE INDEX IF NOT EXISTS idx_proxies_last_check ON proxies(last_check_at);
```

**不建唯一索引**。原因：历史数据可能已有重复，自动合并会删除用户数据并重新指向环境绑定，风险高。由应用层在单事务内查重；按确认**不提供历史重复合并工具**，已有重复数据保持原样，后续导入命中同 `endpoint_key` 时按策略跳过或更新。

### 4.4 迁移脚本 `011_proxy_pool.sql`

```sql
CREATE TABLE IF NOT EXISTS proxy_subscriptions (...);

ALTER TABLE proxies ADD COLUMN group_name TEXT NOT NULL DEFAULT '';
ALTER TABLE proxies ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';
ALTER TABLE proxies ADD COLUMN note TEXT NOT NULL DEFAULT '';
ALTER TABLE proxies ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE proxies ADD COLUMN subscription_id TEXT REFERENCES proxy_subscriptions(id) ON DELETE SET NULL;
ALTER TABLE proxies ADD COLUMN endpoint_key TEXT NOT NULL DEFAULT '';
ALTER TABLE proxies ADD COLUMN weight INTEGER NOT NULL DEFAULT 1;
ALTER TABLE proxies ADD COLUMN last_check_ok INTEGER;
ALTER TABLE proxies ADD COLUMN last_check_message TEXT;
ALTER TABLE proxies ADD COLUMN last_check_city TEXT;
ALTER TABLE proxies ADD COLUMN fail_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE proxies ADD COLUMN disabled_reason TEXT NOT NULL DEFAULT '';

UPDATE proxies
SET endpoint_key =
    lower(trim(protocol)) || '://' ||
    lower(trim(host)) || ':' || port || '|' ||
    trim(username);

CREATE INDEX IF NOT EXISTS idx_proxies_endpoint_key ON proxies(endpoint_key);
-- 其余索引同上
```

同步修改 `src-tauri/src/db/mod.rs` 的迁移列表，追加 `("011_proxy_pool", include_str!("../../migrations/011_proxy_pool.sql"))`。

迁移是纯增量：旧版本代码忽略新列/新表即可回滚，不需要破坏性变更。

### 4.5 `endpoint_key` 规范化规则

统一使用下面的公式（DB 迁移与 Rust `endpoint_key()` 必须完全一致）：

```
lower(trim(protocol)) + "://" + lower(trim(host)) + ":" + port + "|" + trim(username)
```

- `protocol`、`host` 小写；`username` 仅去空格，**不转小写**（部分代理用户名区分大小写）。
- 用户名参与 key，因为同一出口可能有多账号；为空时保留末尾的 `|`。
- 密码不参与 key；订阅刷新时更新密码。
- 同 key 视为同一代理，避免重复导入。
- `host` 按数据库原值参与 key，不主动增删 IPv6 方括号；导入时统一规范化为同一形式。
- `Proxy::url()` 需同步修复 IPv6：host 含 `:` 且未带方括号时自动加 `[]`。

---

## 5. 后端设计

### 5.1 模型类型（`models.rs`）

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    // ...现有字段...
    #[serde(default)] pub group_name: String,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub note: String,
    #[serde(default)] pub source: String,
    #[serde(default)] pub subscription_id: Option<String>,
    #[serde(default)] pub weight: u32,
    #[serde(default)] pub last_check_ok: Option<bool>,
    #[serde(default)] pub last_check_message: Option<String>,
    #[serde(default)] pub last_check_city: Option<String>,
    #[serde(default)] pub fail_count: u32,
    #[serde(default)] pub disabled_reason: String,
    #[serde(default, skip_serializing)] pub endpoint_key: String,
}
```

订阅 API 模型（`ProxySubscription`）返回 `url`、`headers` 明文，不返回 `url_enc` / `headers_enc`；解密只在 service 层读取时发生。

新增输入/输出类型：

- `ProxyFilter`：`search/group/tag/source/subscriptionId/enabled/status/limit/offset`
- `ProxyPage`：`items: Vec<Proxy>, total: u32`
- `ProxyStats`：总数、启用数、可用数、失败数、未检测数、订阅数
- `ProxyImportOptions`：默认协议、启用、分组、标签、名称前缀、重复策略
- `ProxyImportPreview` / `ProxyImportRow` / `ProxyImportError` / `ProxyImportResult`
- `ProxyBatchCheckInput`：`ids`、`filter`、`onlyEnabled`、`concurrency`
- `ProxyJobView` / `ProxyBatchProgress`
- `ProxySubscription` / `ProxySubscriptionInput` / `ProxySubscriptionUpdate`
- `ProxyRefreshResult`

所有新增结构统一 `#[serde(rename_all = "camelCase")]`，与现有前端契约一致。

实现注意：

- `Proxy` 新增字段使用 `#[serde(default)]`；`endpoint_key` 使用 `#[serde(default, skip_serializing)]`。
- 任何创建/更新代理的路径都必须重新计算 `endpoint_key`，包括单条新增/编辑、批量导入、订阅刷新、环境包导入。
- 现有 `Proxy { ... }` 结构体字面量（`services.rs`、`package.rs`、测试）需要补字段或改用 `..Default::default()`，否则编译失败。

### 5.2 解析器（`proxy_import.rs`）

#### 支持格式（P0）

1. **纯文本行**
   - `host:port`
   - `host:port:username:password`
   - `username:password@host:port`
   - `protocol://username:password@host:port`
   - `[2001:db8::1]:8080`（IPv6）
   - `#` 注释行、空行忽略，兼容 CRLF 与 BOM。
2. **Base64 文本**：整体 Base64 解码后若像代理文本，再按行解析。
3. **JSON（P1）**：支持包含 `http/https/socks5` 节点的简单数组；SIP008 的 Shadowsocks 节点不支持，会遇到协议过滤。
4. **Clash YAML（P1）**：只取 `type: http|https|socks5`，读取 `name/server/port/username/password`；`ss/vmess/trojan/hysteria` 计入「不支持协议」并跳过。

#### 解析规则

- 协议默认取导入选项，行内显式协议优先。
- `socks5h` 归一为 `socks5`；未来如需 DNS 远端解析再扩展协议枚举。
- 端口必须 1-65535，主机不能为空。
- `username:password@host:port` 使用最后一个 `@` 分割，避免密码含 `@` 时误判。
- `host:port:username:password` 使用 `splitn(4, ':')`，允许密码含 `:`；IPv6 必须先按方括号识别主机，不能直接按冒号切分。
- URI 形式使用 `url` crate 解析并做百分号解码；`#name` 作为名称。
- Base64 自动识别需加约束：仅当整段文本只含 Base64 字符、长度 ≥ 16、解码后包含 `:` 或 `://` 时才按 Base64 处理，避免把普通短文本误判。
- 行内名称：URI 的 `#name` 或 Clash 的 `name`；否则按 `名称前缀 + 序号` 或 `host:port` 生成。
- 批次内重复：保留第一条，后续标记 `duplicate_in_batch`。
- 库内重复：按 `endpoint_key` 判断；策略 `skip`（默认）/`update`/`error`。
- 单批上限 5000 条；超限直接拒绝，避免一次导入拖垮 UI/DB。
- 错误行不阻断整个导入；全部无效时返回 400。

#### 预览与提交的一致性

- `proxy_import_preview` 被内部分页截断（只返回前 200 行）但统计全量。
- `proxy_import_commit` 接收原始文本和选项，**重新解析**，不信任前端回传的行数据。
- 预览中的密码永远脱敏（`hasPassword`），不返回明文。

### 5.3 批量导入服务

`services::import_proxies(state, text, options, actor)`：

1. 解析并校验。
2. 在**一个事务**内：
   - 按 `endpoint_key` 批量查询已有代理；
   - `skip`：跳过；`update`：更新协议/用户名/密码（保留 `id`、绑定关系、名称；分组/标签仅对新记录应用，避免覆盖用户手工分类）；
   - 新记录：加密密码、生成 `id`、写入 `source='import'`。
3. 返回汇总：`inserted/updated/skipped/failed/errors`。
4. 只写一条审计 `proxy.batch_import`，detail 记录数量与前若干个名称，不记录密码。

DB 层新增：

```rust
pub fn insert_proxies_bulk(&self, proxies: &[Proxy]) -> Result<u32>;
pub fn update_proxies_bulk(&self, proxies: &[Proxy]) -> Result<u32>;
pub fn find_proxies_by_endpoint_keys(&self, keys: &[String]) -> Result<Vec<Proxy>>;
```

同时把 `row_to_proxy` 改为按列名读取，避免新增字段后索引错位；`tags` 用 `serde_json` 解析，损坏时回退为空数组并记录 warning，不让单条脏数据拖垮整个列表。

### 5.4 批量检测与 Job

#### 5.4.1 检测核心拆分

现有 `proxy_check::check_proxy` 同时做检测和写库。为批量复用，拆成：

```rust
pub async fn check_proxy_core(proxy: &Proxy, password: Option<&str>) -> ProxyCheckResult;
pub async fn check_proxy(db: &Db, proxy: &Proxy, password: Option<&str>) -> Result<ProxyCheckResult> {
    let result = check_proxy_core(proxy, password).await;
    db.save_proxy_check(&result)?;
    Ok(result)
}
```

批量任务调用 `check_proxy_core`，最后统一 `save_proxy_checks(&[ProxyCheckResult])`，避免 N 次抢 DB 锁。

#### 5.4.2 Job 注册表（`proxy_jobs.rs`）

```rust
pub struct ProxyJobManager {
    jobs: RwLock<HashMap<String, Arc<ProxyJob>>>,
}

pub struct ProxyJob {
    pub id: String,
    pub kind: String,          // check
    pub state: Mutex<String>,  // running | done | cancelled | failed
    pub progress: Mutex<ProxyBatchProgress>,
    pub cancel: AtomicBool,
    pub results: Mutex<Vec<ProxyCheckResult>>,
    pub created_at: Instant,
}
```

约束：

- 同时最多 4 个批量检测 Job；超过返回 `conflict`。
- Job 完成后保留 30 分钟，之后清理。
- 取消只阻止未开始的任务；已发出的请求允许自然结束，结果照常写回。
- 不新增依赖：并发用 `tokio::sync::Semaphore`，取消用 `AtomicBool` + `tokio::select!`。
- 进度事件由 Job 持有 `AppState`/`AppHandle` 发出；若应用句柄尚未就绪则只更新内存状态，前端轮询兜底。

#### 5.4.3 并发与进度

- `concurrency` 默认 8，范围 1-32。
- 每完成一条：
  - 更新 `completed/ok/failed`；
  - 最近结果环形缓存 50 条；
  - 距上次事件 ≥ 200ms 时 `emit("proxy://batch", progress)`；
  - 最后一条和结束状态强制 emit。
- 每完成 25 条增量写回一次，任务结束/取消时再写回剩余结果，降低应用中途退出导致结果全部丢失的风险。
- 写回失败则 Job 标记 `failed` 并保留已完成结果，前端可重试或重新检测。
- 退出应用时 Job 随进程结束，不做跨重启恢复（代理本身可重新检测）。

#### 5.4.4 结果写回

```sql
UPDATE proxies
SET last_check_at=?2,
    latency_ms=?3,
    exit_ip=?4,
    exit_country=?5,
    exit_timezone=?6,
    last_check_city=?7,
    last_check_ok=?8,
    last_check_message=?9,
    fail_count = CASE WHEN ?8 = 1 THEN 0 ELSE fail_count + 1 END,
    updated_at=?2
WHERE id=?1
```

- 检测成功：保留出口信息，`last_check_ok=1`，`fail_count=0`。
- 检测失败：保留上一次成功出口信息用于参考，但 `last_check_ok=0`，UI 显示失败原因与「上次出口」。
- 批量检测只写一条审计 `proxy.batch_check`，detail 为 `total/ok/failed/cancelled`。

### 5.5 订阅服务（`proxy_subscription.rs`）

#### 5.5.1 拉取

- 仅允许 `http` / `https`。
- 超时：连接 10s、总 20s；重定向最多 5 次。
- 响应体上限 4 MB（先看 `Content-Length`，再兜底限制）。
- 默认 UA：`FinEnv/1.0 (+proxy-subscription)`；可被自定义头覆盖。
- **需要给 reqwest 开启 gzip/brotli/deflate 特性**：当前 `Cargo.toml` 只有 `json/socks/stream/rustls-tls`，很多订阅源会压缩响应。
- 接口可以回显完整 URL/Headers，但日志与审计仍不记录完整值；错误信息只显示 `scheme://host`。

#### 5.5.2 刷新 diff

| 情况 | 处理 |
| --- | --- |
| 订阅内新节点 | 插入，`source='subscription'`、`subscription_id` |
| 库内已有同 `endpoint_key` 且属于本订阅 | 更新协议/用户名/密码；**不覆盖名称、分组、标签，不改变用户手动设置的 enabled** |
| 库内已有同 key 但属于手工/其他订阅 | 默认跳过并计数 `skipped`；避免订阅劫持手工代理 |
| 订阅中消失的节点 | `enabled=0`、`disabled_reason='subscription_missing'`，**不删除** |
| 之前因缺失停用、本次重新出现 | 若 `disabled_reason='subscription_missing'`，恢复 `enabled=1` 并清空原因；用户手动停用的保持停用 |
| 订阅内重复节点 | 保留第一条 |
| 不支持的协议 | 跳过并计数 `unsupported` |
| 全部无效/解析失败 | 订阅标记 `error`，不改动现有代理 |

刷新结果：

```rust
pub struct ProxyRefreshResult {
    pub subscription_id: String,
    pub added: u32,
    pub updated: u32,
    pub unchanged: u32,
    pub disabled: u32,
    pub restored: u32,
    pub skipped: u32,
    pub unsupported: u32,
    pub total_in_feed: u32,
    pub message: String,
}
```

- 应用 diff 在单事务中完成；事务提交后再更新 `last_refresh_*`（或同一事务内）。
- 删除订阅时默认保留代理：`subscription_id` 置 NULL，同时把 `source` 改为 `manual`，避免出现「来源是订阅但订阅已不存在」的脏状态；可选「同时删除代理」并提示绑定环境数。
- 同一订阅并发刷新需加互斥（内存 `HashSet<subscription_id>` 或 Job 注册表），后到请求返回 `conflict`。

#### 5.5.3 自动刷新

- **默认关闭**：只有用户在某条订阅上显式开启 `auto_refresh` 后才参与扫描。
- 在 `lib.rs` 的 setup 中启动一个 tokio 循环，每 60 秒扫描一次。
- 条件：`enabled=1 AND auto_refresh=1 AND (last_refresh_at IS NULL OR last_refresh_at + refresh_interval_min <= now)`。
- 周期可调：默认 360 分钟，最小 5 分钟，前端提供 5 分钟 / 1 小时 / 6 小时 / 12 小时 / 24 小时 / 自定义。
- 串行执行，避免同时请求多个供应商；单次最多刷新 3 个，剩余下轮继续。
- 审计 actor 使用 `"system"`。
- 刷新完成后 `emit("proxy://changed", { reason: "subscription_auto_refresh", subscriptionId })`，前端若在代理页则静默刷新。
- 自动刷新失败只更新订阅状态并 emit 一条通知事件，不弹阻塞对话框。
- `last_refresh_at` 记录「本次尝试时间」，失败也推进，避免网络故障时每分钟重试。
- 自动刷新只在应用运行期间执行；应用关闭后不驻留后台任务。

### 5.10 在线发现（搜索引擎 / GitHub / 友情链接）

入口：代理池工具栏「在线获取」。

输入：

```rust
pub struct ProxyDiscoveryInput {
    pub search_queries: Vec<String>,
    pub engines: Vec<String>,          // bing | duckduckgo | google
    pub github_queries: Vec<String>,
    pub friend_links: Vec<String>,
    pub follow_links: Option<bool>,
    pub default_protocol: Option<String>,
    pub max_results: Option<u32>,
}
```

流程：

1. 并行抓取来源（最多 6 个并发，单请求 12s 超时、2MB 上限）。
2. 搜索页 / 友情链接：提取页面文本中的代理候选，`followLinks` 开启时跟进页面内链接（搜索最多 5 个、友链最多 8 个）。
3. GitHub：匿名仓库搜索 API 取前 5 个仓库，抓取 `README.md`，可选跟进 README 内链接。
4. HTML 清洗、实体解码、正则提取、`proxy_parse` 校验、按「协议 + 主机 + 端口」去重，带凭据优先。
5. 候选节点密码只保存在后端内存 `ProxyDiscoveryStore`（TTL 30 分钟），接口只返回 `hasPassword`。
6. `proxy_discovery_add(candidateIds, options)` 将选中候选写入代理池，`source='discovery'`，复用导入的重复策略与审计。

安全：

- 仅允许 `http/https`，不执行页面脚本。
- 不记录完整页面内容；错误信息不回显凭据。
- 候选结果不返回密码；添加时由后端按 ID 读取内存记录。
- GitHub 匿名 API 有速率限制，失败通过 `sources` / `errors` 返回。

### 5.6 分组、标签、筛选

- `group_name` 是自由文本，不做外键；前端提供已有分组下拉 + 新输入。
- `tags` 为字符串数组，DB 存 JSON；提供 `addTags` / `removeTags` / `setTags` 三种批量操作。
- 筛选条件全部下推到 SQL，避免前端全量加载：

```rust
pub struct ProxyFilter {
    pub search: Option<String>,
    pub group: Option<String>,
    pub tag: Option<String>,
    pub source: Option<String>,
    pub subscription_id: Option<String>,
    pub enabled: Option<bool>,
    pub status: Option<String>, // ok | fail | untested | disabled
    pub disabled_reason: Option<String>, // manual | subscription_missing
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
```

- 默认分页 100 条；`limit` 上限 500。
- 保留 `list_proxies()` 供模板/统计等内部调用。

### 5.7 审计与权限

新增审计动作：

```rust
pub const PROXY_BATCH_IMPORT: &str = "proxy.batch_import";
pub const PROXY_BATCH_UPDATE: &str = "proxy.batch_update";
pub const PROXY_BATCH_DELETE: &str = "proxy.batch_delete";
pub const PROXY_BATCH_CHECK: &str = "proxy.batch_check";
pub const PROXY_SUBSCRIPTION_CREATE: &str = "proxy.subscription_create";
pub const PROXY_SUBSCRIPTION_UPDATE: &str = "proxy.subscription_update";
pub const PROXY_SUBSCRIPTION_DELETE: &str = "proxy.subscription_delete";
pub const PROXY_SUBSCRIPTION_REFRESH: &str = "proxy.subscription_refresh";
```

权限沿用现有模型：

| 操作 | 权限 |
| --- | --- |
| 列表、筛选、单条/批量检测 | `ViewList` |
| 新增/编辑/删除/导入/批量启停/清理停用节点/订阅管理/刷新 | `EditFingerprintProxy` |
| 批量分配环境 | `EditFingerprintProxy` + 每个环境 `EnvAccess::Manage` |
| 导出含密码（P2） | `ExportSensitive` + 二次确认 + 全局高危开关 |

审计 detail 禁止出现密码、完整订阅 URL、Authorization 头。

### 5.8 HTTP API

在现有路由基础上追加：

```text
GET    /proxies                          // 保持旧行为：返回数组
GET    /proxies/page?search=&group=&tag=&source=&subscriptionId=&status=&enabled=&limit=&offset=
GET    /proxies/stats
POST   /proxies/import/preview
POST   /proxies/import
POST   /proxies/batch/check            -> 202 { jobId, total }
GET    /proxy-jobs/{jobId}
POST   /proxy-jobs/{jobId}/cancel
POST   /proxies/batch/enable           { ids, enabled }
POST   /proxies/batch/update           { ids, groupName?, addTags?, removeTags?, note? }
POST   /proxies/batch/delete           { ids } + X-FinEnv-Confirm: true
POST   /proxies/batch/delete-disabled  { subscriptionId?, group?, source?, disabledReason? } + X-FinEnv-Confirm: true
GET    /proxy-subscriptions
POST   /proxy-subscriptions
PATCH  /proxy-subscriptions/{id}
DELETE /proxy-subscriptions/{id}       + X-FinEnv-Confirm: true
POST   /proxy-subscriptions/{id}/refresh
POST   /proxy-discovery/scan           // 候选结果，不返回密码
POST   /proxy-discovery/add            // 选中候选入库
```

兼容性：

- **保留 `GET /proxies` 返回数组的旧行为**；分页/筛选使用新增的 `GET /proxies/page`，避免破坏现有 SDK 与外部脚本。
- 桌面端直接使用 `proxy_query` 获取 `{items,total}`。
- 现有 `POST /proxies`、`PATCH /proxies/{id}`、`DELETE /proxies/{id}`、`POST /proxies/{id}/check` 保持不变。
- 批量接口统一返回摘要 + 逐项错误，HTTP 200；仅当全部失败或参数非法时返回 4xx。
- Job 查询返回 `ProxyJobView`，包含 `state/progress/results`。
- API 列表返回解密后的 `url` / `headers`（按确认可回显明文），但永不返回 `url_enc`、`headers_enc`、`password_enc`。
- `delete-disabled` 只删除 `enabled=0` 的节点，返回 `{deleted, affectedEnvs}`；执行前提示会影响的环境数。

SDK（`sdk/js`、`sdk/python`）在 P2 增加批量导入、批量检测、订阅方法；P0 不强制。

### 5.9 Tauri 命令清单

```text
proxy_list()                         // 保留
proxy_query(filter)                  // 分页/筛选
proxy_stats()
proxy_import_preview(text, options)
proxy_import_commit(text, options)
proxy_batch_check(input)             // -> jobId
proxy_job_get(jobId)
proxy_job_cancel(jobId)
proxy_batch_set_enabled(ids, enabled)
proxy_batch_update(ids, patch)
proxy_batch_delete(ids, confirm)
proxy_batch_delete_disabled(filter, confirm)
proxy_subscription_list()
proxy_subscription_create(input)
proxy_subscription_update(id, input)
proxy_subscription_delete(id, deleteProxies, confirm)
proxy_subscription_refresh(id)       // -> ProxyRefreshResult
proxy_discovery_scan(input)          // -> ProxyDiscoveryResult
proxy_discovery_add(input)           // -> ProxyImportResult
```

在 `lib.rs` 的 `generate_handler!` 中注册。

---

## 6. 前端设计

### 6.1 页面信息架构

`ProxiesPage` 从单页表格改为：

```
代理池
├─ 顶部统计卡片（总数 / 启用 / 可用 / 失败 / 订阅数）
├─ Tab: 代理列表 | 订阅源
├─ 工具栏
│   ├─ 搜索、分组、来源、状态筛选
│   ├─ 在线获取（搜索引擎 / GitHub / 友情链接）
│   ├─ 批量导入
│   ├─ 批量检测（选中/全部）
│   ├─ 清理停用代理
│   ├─ 新增代理
│   └─ 刷新
├─ 表格（复选框、状态、来源、分组/标签、出口、延迟、绑定数、操作）
├─ 批量操作条（选中后浮出）
│   ├─ 启用 / 停用 / 删除
│   ├─ 设置分组 / 添加标签 / 移除标签
│   └─ 检测选中
└─ 批量检测面板（右侧抽屉或底部面板）
    ├─ 进度条 completed/total、成功/失败
    ├─ 最近结果列表
    ├─ 取消
    └─ 完成后「只看失败」「重试失败」
```

订阅源 Tab：

```
订阅源
├─ 新增订阅
├─ 订阅卡片/表格：名称、URL（可查看/复制明文）、代理数、最近刷新、状态、自动刷新开关、周期
├─ 操作：刷新、检测该订阅、编辑、删除
└─ 刷新结果摘要（新增/更新/停用/恢复/跳过/不支持）
```

### 6.2 组件拆分

| 组件 | 职责 |
| --- | --- |
| `pages/Proxies.tsx` | 页面容器、Tab、筛选状态、数据刷新 |
| `components/proxy/ProxyTable.tsx` | 表格、多选、状态/来源/分组展示 |
| `components/proxy/ProxyToolbar.tsx` | 搜索、筛选、批量入口 |
| `components/proxy/ProxyBulkBar.tsx` | 选中后的批量操作 |
| `components/proxy/ProxyEditorDialog.tsx` | 单条新增/编辑（从现文件拆出） |
| `components/proxy/ProxyImportDialog.tsx` | 粘贴/文件、选项、预览、提交 |
| `components/proxy/ProxyBatchCheckPanel.tsx` | Job 进度、取消、结果、重试 |
| `components/proxy/ProxyAssignDialog.tsx` | 批量分配环境（从现文件拆出） |
| `components/proxy/SubscriptionPanel.tsx` | 订阅列表与操作 |
| `components/proxy/SubscriptionEditorDialog.tsx` | 订阅新增/编辑 |
| `hooks/useProxyJob.ts` | 事件订阅 + 轮询兜底 + 清理 |

### 6.3 交互细节

#### 批量导入

- 两种输入：多行文本框（默认）、选择 `.txt/.csv/.json/.yaml` 文件。
- 选项：默认协议、默认启用、分组、标签、名称前缀、重复策略（跳过/更新/报错）。
- 预览：
  - 顶部统计：总行数、有效、批内重复、库内重复、错误、不支持协议；
  - 表格列：行号、协议、地址、用户名、密码状态、重复标记、错误原因；
  - 错误行高亮，可复制；
  - 超过 200 行提示「仅预览前 200 行」。
- 提交按钮显示有效条数；提交中禁用关闭；完成后显示摘要，并提供「检测新增代理」。

#### 批量检测

- 默认只检测启用代理；提供「包含已停用」开关。注意：显式传入的 `ids` 一律检测；`onlyEnabled` 只作用于「按筛选/全部」选择。
- 并发数选择：4 / 8 / 12 / 16 / 32，默认 8。
- 进度面板不阻塞页面；关闭面板后任务继续，顶部显示运行中任务入口。
- 完成后：
  - 成功/失败统计；
  - 「只看失败」筛选；
  - 「重试失败」再次创建 Job；
  - 列表自动刷新。
- 事件丢失时用 `proxy_job_get` 每 2 秒轮询兜底；Job 结束后停止。

#### 订阅

- 新建时首次刷新可勾选「创建后立即刷新」；自动刷新默认关闭，需要用户显式开启并选择周期。
- URL 编辑：输入框回显明文，可直接复制；留空表示保持原 URL（编辑时）。
- 请求头使用高级 JSON 文本框，回显明文；保存前校验 JSON 对象。
- 删除确认：
  - 默认「保留代理，转为手工」；
  - 可选「同时删除该订阅的 N 个代理」，提示会影响 M 个环境绑定。
- 刷新失败时在订阅行展示 `last_refresh_message`，不弹全局错误。
- 订阅因缺失而停用的节点在列表中显示「订阅缺失」标记，可被「清理停用代理」批量删除。

#### 状态展示

- 优先 `lastCheckOk`：
  - `true` → 可用（绿色）
  - `false` → 失败（红色，tooltip 显示 `lastCheckMessage`）
  - `null` → 未检测（灰色）
- 停用 → 灰色「已停用」，优先于检测状态。
- 出口列显示 `exitIp / exitCountry / exitCity / exitTimezone` 与最近检测时间；失败时额外显示「上次成功出口」。

### 6.4 IPC 类型与事件

`frontend/src/ipc/types.ts` 扩展：

```ts
export interface Proxy {
  // ...现有字段...
  groupName: string;
  tags: string[];
  note: string;
  source: 'manual' | 'import' | 'subscription' | string;
  subscriptionId: string | null;
  weight: number;
  lastCheckOk: boolean | null;
  lastCheckMessage: string | null;
  lastCheckCity: string | null;
  failCount: number;
  disabledReason: '' | 'manual' | 'subscription_missing' | string;
}

export interface ProxySubscription {
  id: string;
  name: string;
  url: string;              // 解密后的明文
  headers: string | null;   // 解密后的明文 JSON
  defaultProtocol: 'http' | 'https' | 'socks5';
  enabled: boolean;
  autoRefresh: boolean;     // 默认 false
  refreshIntervalMin: number;
  lastRefreshAt: string | null;
  lastRefreshStatus: string | null;
  lastRefreshMessage: string | null;
  proxyCount: number;
  createdAt: string;
  updatedAt: string;
}
```

新增事件：

```ts
export interface ProxyBatchProgress {
  jobId: string;
  kind: 'check' | string;
  state: 'running' | 'done' | 'cancelled' | 'failed';
  total: number;
  completed: number;
  ok: number;
  failed: number;
  startedAt: string;
  finishedAt: string | null;
  message: string;
  recent: ProxyCheckResult[];
}

export interface ProxyChangedPayload {
  reason: string;
  subscriptionId?: string;
}
```

`ipc/client.ts`：

```ts
proxyQuery: (filter: ProxyFilter) => call<ProxyPage>('proxy_query', { filter }),
proxyImportPreview: (input) => call<ProxyImportPreview>('proxy_import_preview', input),
proxyImportCommit: (input) => call<ProxyImportResult>('proxy_import_commit', input),
proxyBatchCheck: (input) => call<{ jobId: string; total: number }>('proxy_batch_check', input),
proxyJobGet: (jobId) => call<ProxyJobView>('proxy_job_get', { jobId }),
proxyJobCancel: (jobId) => call<void>('proxy_job_cancel', { jobId }),
proxyBatchDeleteDisabled: (filter, confirm) =>
  call<{ deleted: number; affectedEnvs: number }>('proxy_batch_delete_disabled', { filter, confirm }),
proxySubscriptionList: () => call<ProxySubscription[]>('proxy_subscription_list'),
proxySubscriptionCreate: (input) => call<ProxySubscription>('proxy_subscription_create', { input }),
proxySubscriptionUpdate: (id, input) => call<ProxySubscription>('proxy_subscription_update', { id, input }),
proxySubscriptionDelete: (id, deleteProxies, confirm) =>
  call<void>('proxy_subscription_delete', { id, deleteProxies, confirm }),
proxySubscriptionRefresh: (id) => call<ProxyRefreshResult>('proxy_subscription_refresh', { id }),
onProxyBatch: (handler) => listen<ProxyBatchProgress>('proxy://batch', ...),
onProxyChanged: (handler) => listen<ProxyChangedPayload>('proxy://changed', ...),
```

---

## 7. 安全设计

1. **凭据加密与回显**
   - 代理密码继续逐条 AES-256-GCM。
   - 订阅 URL、订阅请求头仍使用同一 `CryptoManager` 加密落库；读取时解密，列表/编辑接口返回明文 `url` / `headers`（按确认）。
   - 如后续希望完全明文落库，只需去掉 `url_enc` / `headers_enc` 的加解密层，不影响接口形状。
2. **日志脱敏**
   - 禁止记录代理密码、完整订阅 URL、Authorization 头。
   - 错误信息只拼接 `scheme://host`，不回显路径和查询串。
3. **审计**
   - 批量操作汇总一条，detail 不含凭据。
   - 订阅刷新 actor 为登录用户或 `system`。
4. **导入**
   - 后端重新解析，不信任前端预览结果。
   - 单批上限 5000，响应体上限 4 MB，防止内存/DB 压力。
5. **SSRF**
   - 桌面端允许用户访问内网自建订阅，但仅限 `http/https`；禁止 `file://`、`ftp://` 等。
   - 重定向后再次校验 scheme。
6. **导出**
   - 默认导出不含密码；P2 的「含密码导出」复用 `ExportSensitive` + `X-FinEnv-Confirm` + 全局高危开关。
7. **API**
   - 批量/订阅写操作需要 `EditFingerprintProxy`；列表/检测需要 `ViewList`。
   - 删除类操作继续要求确认头/参数。

---

## 8. 性能与容量

| 场景 | 目标 |
| --- | --- |
| 代理总量 | 10,000 条以内 |
| 单次导入 | 5,000 条以内，事务批量写入 |
| 单次批量检测 | 2,000 条以内；8 并发 |
| 列表默认分页 | 100 条，最大 500 |
| 订阅响应体 | 4 MB 上限 |
| 单订阅节点数 | 5,000 条上限，超出截断并提示 |
| 进度事件 | 200ms 节流，避免事件风暴 |
| DB 锁 | 网络请求期间不持锁；检测结果攒批写回 |

注意事项：

- `Db` 是单连接 + Mutex，批量写必须用事务，禁止在循环里逐条 `insert_proxy`。
- 批量检测期间不要持有 `Db` 锁；只在开始时读取代理列表、结束时写回。
- 大列表前端使用分页；表格不做全量渲染。
- 订阅自动刷新串行化，避免同时拉取多个大文件。

---

## 9. 兼容与迁移风险

| 风险 | 处理 |
| --- | --- |
| 旧前端读新字段 | 新字段可选/有默认值，旧代码忽略 |
| 旧 SDK 调 `GET /proxies` | 保留原数组返回，分页接口另开 |
| 迁移时已有重复代理 | 不合并、不提供合并工具；应用层去重，历史重复保持原样 |
| 迁移中断 | 011 为纯增量；失败可回滚事务，旧版本可继续启动 |
| 订阅删除导致绑定丢失 | 默认保留代理；删除代理时依赖现有 `ON DELETE SET NULL`，并提示影响环境数 |
| 自动刷新网络失败 | 只更新订阅状态，不影响已有代理 |
| 名称被订阅覆盖 | 订阅刷新不覆盖名称；新节点才使用订阅名称 |
| 用户手动停用的代理被重新启用 | 刷新时保留现有 `enabled`，只对新增节点应用默认启用 |
| reqwest 不支持 gzip | 在 P0 同步开启 `gzip/brotli/deflate` 特性 |
| IPv6 地址 | 统一 `url_host()` 辅助函数，修复 `Proxy::url()` |

---

## 10. 测试与验收

### 10.1 单元测试（Rust）

- `proxy_import`：
  - 纯文本 5 种格式；
  - IPv6；
  - 密码含 `:` / `@`；
  - Base64 整体编码；
  - 注释/空行/BOM/CRLF；
  - 批内重复、库内重复、非法端口；
  - 超过 5000 条拒绝。
- `endpoint_key` 规范化与大小写。
- 订阅 diff：
  - 新增/更新/消失；
  - 消失节点停用并标记，不删除；
  - 缺失节点恢复时重新启用，用户手动停用的保持停用；
  - 手工代理冲突跳过；
  - 不支持协议计数；
  - 刷新不覆盖名称、分组、标签与手动启停状态。
- DB：
  - 迁移幂等；
  - 分页筛选 SQL；
  - 批量插入/更新事务；
  - 检测结果批量写回与 `fail_count` 清零。
- Job：
  - 并发上限；
  - 取消；
  - 进度汇总；
  - Job 过期清理。

### 10.2 前端测试

- 类型检查、Vitest、生产构建。
- `ProxyImportDialog` 预览统计渲染。
- `ProxyBatchCheckPanel` 进度/取消/失败筛选。
- `useProxyJob` 事件 + 轮询兜底。

### 10.3 手工验收场景

1. 粘贴 100 条混合格式代理，预览能标出 3 条错误；导入 95 条；重复导入同批全部跳过。
2. 选中 50 条批量检测，进度实时更新；中途取消；已完成结果落库；重启应用后失败状态仍为失败。
3. 创建订阅（Base64 文本），首次刷新新增 20 条；再次刷新无重复；从源中删除 1 条后显示「停用 / 订阅缺失」，环境绑定仍保留；该节点重新出现后恢复启用。
4. 自动刷新默认关闭；手动开启并设为 5 分钟后，到期执行，前端收到 `proxy://changed` 并刷新。
5. 删除订阅默认保留代理并转为手工；选择同时删除时提示绑定环境数。
6. 批量设置分组/标签后，筛选和统计正确；「清理停用代理」能删除停用节点并提示影响的环境数。
7. HTTP API：创建 Job、轮询、取消、拉取结果；代理列表不返回密码；订阅接口按确认返回明文 URL/Headers。
8. 审计：批量操作只有一条汇总记录，详情不含凭据或完整订阅 URL。

### 10.4 校验命令

```bash
npm --prefix frontend run check
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## 11. 分阶段实施计划

### P0：基础批量能力（建议先做）

- 迁移 011、`Proxy` 新字段、DB 映射/分页筛选/批量写入。
- `proxy_import.rs`（纯文本 + Base64 + URI + 行内错误 + 去重）。
- 批量导入预览/提交。
- 批量检测 Job、并发、进度事件、取消、结果持久化、失败状态修复。
- 前端：多选、批量操作条、导入弹窗、检测面板、状态展示。
- 「清理停用代理」批量删除。
- 审计与权限。
- 单元测试 + 前端校验。

### P1：订阅源

- `proxy_subscriptions` 表与 CRUD。
- 拉取、解析、diff、缺失停用/恢复、手动刷新、可选自动刷新循环（默认关闭）。
- 订阅管理 UI。
- URL/Headers 解密回显（落库仍加密）。
- Clash YAML（仅 http/https/socks5）与 JSON 简单数组。
- 批量检测按订阅筛选。
- HTTP API 订阅接口。

### P2：增强与治理

- 分组/标签批量编辑（如果 P0 未包含）。
- 模板代理池按分组/来源/权重轮询。
- 导出（脱敏/含密码）。
- SDK 批量/订阅方法。
- 检测历史趋势、失败自动停用（默认关闭）。
- 订阅刷新历史记录。

> 建议 P0 先交付，确认交互后再做 P1；订阅依赖批量导入和检测的稳定性。

### 11.1 粗略工作量（单人估算）

| 阶段 | 后端 | 前端 | 测试/文档 | 合计 |
| --- | --- | --- | --- | --- |
| P0 | 2-3 天 | 2-3 天 | 1-2 天 | 5-8 人日 |
| P1 | 2-3 天 | 1-2 天 | 1 天 | 4-6 人日 |
| P2 | 2-4 天 | 2-3 天 | 1 天 | 5-8 人日 |

实际耗时取决于订阅格式兼容范围、UI 细节和测试环境中的代理可用性。

---

## 12. 已采用的默认项

前 5 项已在第 0.1 节确认；其余项按下列默认实现：

| # | 实现 |
| --- | --- |
| 1 | 订阅 URL/Headers 落库保持 AES 加密，接口回显明文 |
| 2 | 批量导入由后端解析，前端只做预览 |
| 3 | 批量检测默认并发 8，范围 1-32 |
| 4 | 分组/标签已放入 P0 并实现 |
| 5 | 支持纯文本/Base64/URI/JSON/Clash HTTP/SOCKS5 节点；SS 等协议跳过并计数 |
| 6 | HTTP API 已同步；SDK 留 P2 |
| 7 | 单批导入 5000、单订阅 5000、批量检测 2000 上限 |
| 8 | 代理列表后端分页，默认 100 条 |

---

## 13. 附录

### 13.1 支持的文本格式示例

```text
# 注释
1.2.3.4:8080
1.2.3.4:8080:user:pass
user:pass@1.2.3.4:8080
http://user:pass@1.2.3.4:8080
socks5://user:pass@proxy.example.com:1080
[2001:db8::1]:8080
```

### 13.2 Base64 订阅示例

原文：

```text
socks5://user:pass@1.2.3.4:1080
http://5.6.7.8:8080
```

Base64 后整体提交，后端自动检测并解码。

### 13.3 订阅 diff 示例

刷新前：

```text
A: 1.1.1.1:8080
B: 2.2.2.2:8080
C: 3.3.3.3:8080
```

刷新后：

```text
A: 1.1.1.1:8080（密码变化）
B: 2.2.2.2:8080
D: 4.4.4.4:8080
```

结果：

```json
{
  "added": 1,
  "updated": 1,
  "unchanged": 1,
  "disabled": 1,
  "restored": 0,
  "skipped": 0,
  "unsupported": 0,
  "totalInFeed": 3
}
```

### 13.4 批量检测事件示例

```json
{
  "jobId": "job_01",
  "kind": "check",
  "state": "running",
  "total": 50,
  "completed": 12,
  "ok": 10,
  "failed": 2,
  "startedAt": "2026-11-27T10:00:00Z",
  "finishedAt": null,
  "message": "正在检测 12/50",
  "recent": [
    {
      "proxyId": "proxy-uuid",
      "ok": false,
      "latencyMs": 8000,
      "exitIp": null,
      "country": null,
      "timezone": null,
      "city": null,
      "message": "TCP 连接超时",
      "checkedAt": "2026-11-27T10:00:05Z"
    }
  ]
}
```

### 13.5 现有代码改动索引

| 文件 | 改动 |
| --- | --- |
| `src-tauri/migrations/011_proxy_pool.sql` | 新增表/字段/索引 |
| `src-tauri/src/db/mod.rs` | 注册迁移 |
| `src-tauri/src/db/proxy.rs` | 新字段映射、分页筛选、批量写入、订阅 CRUD |
| `src-tauri/src/models.rs` | Proxy 扩展、导入/Job/订阅类型 |
| `src-tauri/src/proxy_import.rs` | 新增解析器 |
| `src-tauri/src/proxy_subscription.rs` | 新增订阅服务 |
| `src-tauri/src/proxy_discovery.rs` | 新增在线发现（搜索引擎 / GitHub / 友链） |
| `src-tauri/src/proxy_jobs.rs` | 新增 Job 管理 |
| `src-tauri/src/proxy_check.rs` | 拆分检测核心与持久化 |
| `src-tauri/src/services.rs` | 批量导入/检测/订阅编排、审计 |
| `src-tauri/src/commands/mod.rs` | 新命令 |
| `src-tauri/src/api_server/mod.rs` | 新路由/处理器 |
| `src-tauri/src/audit_log.rs` | 新审计动作 |
| `src-tauri/src/state.rs` | 挂载 Job 管理器 |
| `src-tauri/src/lib.rs` | 注册命令、启动自动刷新循环 |
| `src-tauri/Cargo.toml` | reqwest 开启 gzip/brotli/deflate；Clash YAML 使用内置轻量解析，无额外依赖 |
| `frontend/src/ipc/types.ts` | 新类型/事件 |
| `frontend/src/ipc/client.ts` | 新命令/事件封装 |
| `frontend/src/pages/Proxies.tsx` | 页面重构：列表/订阅 Tab、导入、批量检测、批量管理 |
| `docs/api-docs.md` | 新接口文档 |
| `docs/verification.md` | 验收记录 |

---

## 14. 备注

- V2.0 已完成 P0/P1 主体实现：迁移、解析器、批量导入、批量检测 Job、订阅 CRUD/刷新/自动刷新、在线发现（搜索引擎 / GitHub / 友链）、批量管理、前端页面与 HTTP API。
- P2 项（模板按分组/权重轮询、导出、SDK 批量/订阅方法、检测历史趋势等）尚未实现。
- `proxy_subscriptions` 实际增加了 `group_name` / `tags`，用于给订阅新增节点设置默认分组和标签。
- 订阅缺失节点统一停用并标记 `disabled_reason='subscription_missing'`，重新出现时自动恢复；手动停用的节点不会被自动恢复。
- 订阅 URL/Headers 落库仍使用 AES 加密，列表/编辑接口返回明文；日志与审计不记录完整值。
- 历史重复代理不做合并，也不提供合并工具；应用层按 `endpoint_key` 去重。

## 15. 实施文件

后端：

- `src-tauri/migrations/011_proxy_pool.sql`
- `src-tauri/src/proxy_import.rs`
- `src-tauri/src/proxy_jobs.rs`
- `src-tauri/src/proxy_subscription.rs`
- `src-tauri/src/proxy_discovery.rs`
- `src-tauri/src/db/proxy.rs`
- `src-tauri/src/models.rs`
- `src-tauri/src/services.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/api_server/mod.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/audit_log.rs`
- `src-tauri/Cargo.toml`

前端：

- `frontend/src/ipc/types.ts`
- `frontend/src/ipc/client.ts`
- `frontend/src/pages/Proxies.tsx`
