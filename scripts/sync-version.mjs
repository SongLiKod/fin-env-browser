// Keep every user-visible/build-time version in sync with the root VERSION file.
// Usage:
//   node scripts/sync-version.mjs          update generated manifests
//   node scripts/sync-version.mjs --check  fail if any manifest is out of date
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const check = process.argv.includes('--check');
const version = readFileSync(join(root, 'VERSION'), 'utf8').trim();

if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  console.error(`VERSION 不是合法语义化版本号: ${version}`);
  process.exit(1);
}

const docVersion = `V${version.split('.').slice(0, 2).join('.')}`;
const mismatches = [];

function replaceFirstJsonVersion(text, count = 1) {
  let seen = 0;
  return text.replace(/("version"\s*:\s*")[^"]*(")/g, (match, start, end) => {
    seen += 1;
    return seen <= count ? `${start}${version}${end}` : match;
  });
}

function update(relative, transform) {
  const path = join(root, relative);
  let before;
  try {
    before = readFileSync(path, 'utf8');
  } catch (error) {
    mismatches.push(`${relative} 无法读取: ${error.message}`);
    return;
  }
  const after = transform(before);
  if (after === before) return;
  if (check) mismatches.push(relative);
  else writeFileSync(path, after, 'utf8');
}

const edits = [
  ['package.json', (text) => replaceFirstJsonVersion(text, 1)],
  ['frontend/package.json', (text) => replaceFirstJsonVersion(text, 1)],
  ['frontend/package-lock.json', (text) => replaceFirstJsonVersion(text, 2)],
  ['sdk/js/package.json', (text) => replaceFirstJsonVersion(text, 1)],
  ['sdk/js/package-lock.json', (text) => replaceFirstJsonVersion(text, 2)],
  ['src-tauri/tauri.conf.json', (text) => replaceFirstJsonVersion(text, 1)],
  [
    'src-tauri/Cargo.toml',
    (text) => text.replace(/(\[package\][\s\S]*?^version = ")[^"]+(")/m, `$1${version}$2`),
  ],
  [
    'src-tauri/Cargo.lock',
    (text) =>
      text.replace(
        /(\[\[package\]\]\r?\nname = "fin-env-browser"\r?\nversion = ")[^"]+(")/,
        `$1${version}$2`,
      ),
  ],
  ['sdk/python/pyproject.toml', (text) => text.replace(/^version = "[^"]+"/m, `version = "${version}"`)],
  ['sdk/python/finenv/__init__.py', (text) => text.replace(/__version__ = "[^"]+"/, `__version__ = "${version}"`)],
  ['docs/fin-env-tech.md', (text) => text.replace(/文档版本：V[0-9.]+/, `文档版本：${docVersion}`)],
  ['docs/fin-env-requirement.md', (text) => text.replace(/文档版本：V[0-9.]+/, `文档版本：${docVersion}`)],
  [
    'docs/versioning.md',
    (text) =>
      text.replace(
        /(当前版本：[\s\S]{0,30}?```text\r?\n)[^\r\n]+(\r?\n```)/,
        `$1${version}$2`,
      ),
  ],
  [
    'docs/api-docs.md',
    (text) =>
      text.replace(
        /("app": "FinEnv", "version": ")[^"]+(")/,
        `$1${version}$2`,
      ),
  ],
  [
    'docs/verification.md',
    (text) =>
      text.replace(
        /("app":"FinEnv","status":"ok","version":")[^"]+(")/,
        `$1${version}$2`,
      ),
  ],
];

for (const [relative, transform] of edits) update(relative, transform);

if (mismatches.length > 0) {
  console.error(`版本号未同步到 ${version}:`);
  for (const file of mismatches) console.error(`  - ${file}`);
  console.error('\n运行 `npm run version:sync` 更新，或只修改根目录 VERSION 文件。');
  process.exit(1);
}

console.log(`${check ? '版本号一致' : '版本号已同步'}: ${version}`);
