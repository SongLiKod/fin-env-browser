# 版本号管理

FinEnv 的版本号只有一个手工维护位置：

```text
VERSION
```

当前版本：

```text
3.1.0
```

## 修改版本

只修改根目录的 `VERSION` 文件，然后运行：

```bash
npm run version:sync
```

`sync-version.mjs` 会同步以下位置，禁止手工修改这些副本：

- 根目录 `package.json`
- `frontend/package.json`、`frontend/package-lock.json`
- `sdk/js/package.json`、`sdk/js/package-lock.json`
- `sdk/python/pyproject.toml`、`sdk/python/finenv/__init__.py`
- `src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`
- `src-tauri/tauri.conf.json`
- 文档中的当前版本号和健康检查示例

`npm run build`、`cargo tauri dev/build` 的 Tauri 前置命令和 `scripts/build.ps1` 都会自动执行同步。

## 校验

```bash
npm run version:check
```

该命令不修改文件，只检查所有副本是否与 `VERSION` 一致；不一致时退出码为 1。CI 的「Version consistency」步骤和 `npm run verify` 也会执行此检查。

`src-tauri/build.rs` 在编译前额外校验 `VERSION` 与 `Cargo.toml`，避免运行时版本与安装包版本不一致。

## 发布

构建产物的版本、窗口/登录页/侧栏/设置页/API 健康检查显示的应用版本均来自同一版本链：

```text
VERSION
  ├─ src-tauri/Cargo.toml → CARGO_PKG_VERSION → 运行时 app_info / API / 备份
  ├─ src-tauri/tauri.conf.json → 安装包版本
  ├─ frontend/package.json → 前端包元数据
  ├─ sdk/js、sdk/python → SDK 版本
  └─ docs → 当前版本文档
```

GitHub Release 的 `v*` tag 只用于发布命名；tag 版本应与 `VERSION` 保持一致。
