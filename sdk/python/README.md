# FinEnv Python SDK

Python 3.8+ 同步客户端，封装 FinEnv REST API 与 Playwright CDP 连接。无必装第三方依赖。

```bash
pip install -e sdk/python
# 可选：使用 Playwright 辅助
pip install -e "sdk/python[playwright]"
pip install playwright && playwright install chromium
```

```python
from finenv import FinEnvClient

client = FinEnvClient.from_env()  # FINENV_BASE_URL / FINENV_API_KEY

env = client.list_environments(search="订单")[0]
runtime = client.open_environment(env["id"], url="https://test.example.com/login")
print("PID", runtime["pid"], "CDP", runtime["debugPort"])

client.save_screenshot(env["id"], "login.png")
cookies = client.get_cookies(env["id"])
print(len(cookies["cookies"]), "cookies")

client.execute(env["id"], "localStorage.setItem('token', 'ci-token')")
print(client.detect_fingerprint(env["id"])["score"])

client.close_environment(env["id"])
```

## Playwright

```python
playwright, browser = client.connect_playwright(env["id"])
context = browser.contexts[0]
page = context.pages[0]
page.goto("https://test.example.com")
print(page.title())
browser.close()
playwright.stop()
```

## 高危接口

删除环境 / 代理、导出 Cookie 等接口需要 API Key 对应角色权限。SDK 自动附加：

- `Authorization: Bearer finenv_xxx`
- 删除环境 / 代理时附加 `X-FinEnv-Confirm: true`

如管理员在设置中关闭「允许高危导出」，Cookie 读取与账号导出会返回 403。

## 错误处理

```python
from finenv import FinEnvError
try:
    client.open_environment(env_id)
except FinEnvError as error:
    print(error.status, error.code, error.message)
```

完整接口与角色权限见 `docs/api-docs.md`。
