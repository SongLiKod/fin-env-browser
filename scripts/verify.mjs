// Static cross-check between the two specification documents and the codebase.
// Run: node scripts/verify.mjs
import { readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const errors = [];
const warnings = [];
const ok = (message) => console.log(`  ✓ ${message}`);
const fail = (message) => {
  errors.push(message);
  console.error(`  ✗ ${message}`);
};

function read(relative) {
  const path = join(root, relative);
  if (!existsSync(path)) {
    fail(`缺少文件 ${relative}`);
    return '';
  }
  return readFileSync(path, 'utf8');
}

console.log('FinEnv 需求/实现静态校验\n');

console.log('[1] 文档与仓库结构');
for (const file of [
  'docs/fin-env-requirement.md',
  'docs/fin-env-tech.md',
  'docs/api-docs.md',
  'docs/verification.md',
  'README.md',
  'frontend/vite.config.ts',
  'frontend/tailwind.config.js',
  'frontend/src/utils/index.ts',
  'frontend/src/utils/dialog.ts',
  'src-tauri/tauri.conf.json',
  'src-tauri/capabilities/default.json',
  'src-tauri/migrations/001_init.sql',
  'src-tauri/migrations/002_hardening.sql',
  'src-tauri/migrations/003_environment_identity.sql',
  'src-tauri/src/process/mod.rs',
  'src-tauri/src/cdp/mod.rs',
  'src-tauri/src/db/mod.rs',
  'src-tauri/src/crypto/mod.rs',
  'src-tauri/src/audit_log.rs',
  'src-tauri/src/api_server/mod.rs',
  'src-tauri/src/capability/mod.rs',
  'sdk/js/src/index.ts',
  'sdk/python/finenv/client.py',
  'chromium-bin/README.md',
  '.github/workflows/build.yml',
  '.gitea/workflows/ci.yml',
]) {
  if (existsSync(join(root, file))) ok(file);
  else fail(`缺少文件 ${file}`);
}

console.log('\n[2] Tauri 命令注册');
const commandsSource = read('src-tauri/src/commands/mod.rs');
const libSource = read('src-tauri/src/lib.rs');
const defined = new Set(
  [...commandsSource.matchAll(/#\[tauri::command\]\s*pub (?:async )?fn ([a-z0-9_]+)/g)].map((m) => m[1]),
);
const registered = new Set(
  [...libSource.matchAll(/commands::([a-z0-9_]+)/g)].map((m) => m[1]),
);
for (const name of defined) if (!registered.has(name)) fail(`命令 ${name} 已定义但未注册`);
for (const name of registered) if (!defined.has(name)) fail(`命令 ${name} 已注册但不存在`);
ok(`定义 ${defined.size} 个 Tauri 命令，注册 ${registered.size} 个`);

console.log('\n[3] 前端 IPC 命令覆盖');
const clientSource = read('frontend/src/ipc/client.ts');
const used = new Set(
  [...clientSource.matchAll(/call(?:<[^>]*>)?\('([a-z0-9_]+)'/g)].map((m) => m[1]),
);
for (const name of used) if (!defined.has(name)) fail(`前端调用了不存在的命令 ${name}`);
ok(`前端调用 ${used.size} 个命令，全部已定义`);
const notUsed = [...defined].filter((name) => !used.has(name));
if (notUsed.length) warnings.push(`仅在特定路径使用的命令：${notUsed.join(', ')}`);

console.log('\n[4] REST API 路由');
const apiSource = read('src-tauri/src/api_server/mod.rs');
const routes = [...apiSource.matchAll(/\.route\(\s*"([^"]+)"/g)].map((m) => m[1]);
const requiredRoutes = [
  '/envs',
  '/envs/:id',
  '/envs/:id/open',
  '/envs/:id/close',
  '/envs/:id/screenshot',
  '/envs/:id/cookies',
  '/envs/:id/fingerprint',
  '/envs/:id/snapshot',
  '/envs/:id/restore-snapshot',
  '/envs/:id/cdp',
  '/envs/:id/proxy',
  '/envs/:id/detect',
  '/proxies',
  '/proxies/:id/check',
  '/audit-logs',
  '/audit-logs/verify',
];
for (const route of requiredRoutes) if (!routes.includes(route)) fail(`缺少 API 路由 ${route}`);
ok(`${routes.length} 条 Axum 路由，关键接口齐全`);
const docs = read('docs/api-docs.md');
for (const required of ['POST /envs', 'POST /envs/{id}/open', 'GET /envs/{id}/screenshot', 'PUT /envs/{id}/cookies', 'PUT /envs/{id}/fingerprint', 'POST /envs/{id}/restore-snapshot']) {
  if (!docs.includes(required)) fail(`API 文档缺少 ${required}`);
}
ok('API 文档覆盖环境 CRUD / 打开关闭 / 截图 / Cookie / 指纹 / 快照');

console.log('\n[5] 数据库表与迁移');
const schema = read('src-tauri/migrations/001_init.sql');
for (const table of [
  'groups', 'environments', 'accounts', 'fingerprint_snapshots', 'proxies', 'audit_logs',
  'theme_config', 'users', 'settings', 'api_keys', 'extensions', 'env_extensions',
  'hosts_entries', 'mock_rules', 'captcha_events', 'env_cookies', 'browser_sessions',
  'fingerprint_reports',
]) {
  if (!new RegExp(`CREATE TABLE IF NOT EXISTS ${table}\\b`).test(schema)) fail(`缺少数据表 ${table}`);
}
if (!/audit_logs_no_update/.test(schema) || !/audit_logs_no_delete/.test(schema)) fail('审计不可篡改触发器缺失');
const identity = read('src-tauri/migrations/003_environment_identity.sql');
for (const column of ['border_color', 'ip_address', 'ip_country', 'ip_checked_at']) {
  if (!identity.includes(column)) fail(`环境身份迁移缺少 ${column}`);
}
ok('全部核心表、审计触发器、环境身份字段存在');

console.log('\n[6] 安全与主题令牌');
const capability = read('src-tauri/capabilities/default.json');
if (!capability.includes('core:default') || !capability.includes('dialog:default')) fail('Capability 缺少基础权限');
if (/shell:|fs:|http:/.test(capability)) fail('Capability 不应包含 shell/fs/http 权限');
ok('Capability 最小权限（核心 + 文件对话框）');
const css = read('frontend/src/index.css');
for (const token of ['--color-primary', '--bg-base', '--bg-secondary', '--text-main', '--text-weak', '--border', '--danger', '--success', '--warning', '--radius-sm', '--radius-md', '--radius-lg', '--text-on-primary']) {
  if (!css.includes(token)) fail(`缺少主题 Token ${token}`);
}
if (!/data-theme='dark'/.test(css)) fail('缺少暗色主题 Token');
if (!/data-density='compact'/.test(css)) fail('缺少紧凑密度 Token');
ok('CSS Token / 暗色 / 密度完整');

console.log('\n[7] 需求文档关键能力');
const requirement = read('docs/fin-env-requirement.md');
const tech = read('docs/fin-env-tech.md');
for (const phrase of ['指纹快照', '代理池', 'REST API', 'AES256', '审计日志', '主题配置', '资源管控']) {
  if (!requirement.includes(phrase)) fail(`需求文档缺少「${phrase}」`);
}
for (const phrase of ['AES256-GCM', '主题自定义', 'CDP', 'Axum', 'rusqlite']) {
  if (!tech.toLowerCase().includes(phrase.toLowerCase())) fail(`技术文档缺少「${phrase}」`);
}
ok('需求/技术文档关键能力可追踪');

console.log('\n[8] 产物完整性');
for (const file of ['frontend/dist/index.html', 'sdk/js/dist/index.js', 'sdk/js/dist/index.d.ts']) {
  if (!existsSync(join(root, file))) warnings.push(`尚未构建：${file}`);
}
if (warnings.length) {
  console.log('\n提示：');
  warnings.forEach((warning) => console.log(`  · ${warning}`));
}
console.log(`\n结果：${errors.length === 0 ? '通过' : `${errors.length} 个错误`}，${warnings.length} 个提示`);
process.exit(errors.length === 0 ? 0 : 1);
