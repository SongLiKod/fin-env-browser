# FinEnv JavaScript / TypeScript SDK

Node.js 18+ / 浏览器可用（screenshot 返回 ArrayBuffer）。封装 FinEnv REST API 与 Playwright CDP 连接。

```bash
npm install ./sdk/js
# 或从仓库构建
cd sdk/js && npm install && npm run build
```

```ts
import FinEnvClient from '@finenv/sdk';

const client = FinEnvClient.fromEnv(); // FINENV_BASE_URL / FINENV_API_KEY

const [env] = await client.listEnvironments({ search: '订单' });
const runtime = await client.openEnvironment(env.id, {
  url: 'https://test.example.com/login',
  preScript: "localStorage.setItem('token', 'ci-token')",
});
console.log(runtime.pid, runtime.debugPort);

const cdp = await client.cdpInfo(env.id);
console.log(cdp.httpEndpoint, cdp.wsEndpoint);

const png = await client.screenshot(env.id);
await client.saveScreenshot(env.id, 'login.png');

const cookies = await client.getCookies(env.id);
console.log(cookies.cookies.length);

await client.closeEnvironment(env.id);
```

## Playwright

```bash
npm install playwright
npx playwright install chromium
```

```ts
const { browser, context, cdp } = await client.connectPlaywright(env.id);
const page = context.pages()[0] ?? (await context.newPage());
await page.goto('https://test.example.com');
console.log(await page.title());
await browser.close();
```

也可直接使用 CDP：

```ts
import { chromium } from 'playwright';
const browser = await chromium.connectOverCDP(cdp.httpEndpoint);
```

## 接口

- 环境：list / get / create / update / delete / open / close / status
- 调试：screenshot / cookies / navigate / execute / cdpInfo / recording / HAR
- 指纹：updateFingerprint / detectFingerprint / snapshots
- 网络：proxy / hosts / mocks
- 安全：审计查询与哈希链校验

删除环境、删除代理自动附加 `X-FinEnv-Confirm: true`。完整字段与权限见 `docs/api-docs.md`。
