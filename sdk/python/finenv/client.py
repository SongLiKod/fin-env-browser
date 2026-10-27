"""FinEnv HTTP + CDP SDK (Python 3.8+).

The SDK has no mandatory third-party dependencies. Playwright is optional and
only imported by :meth:`FinEnvClient.connect_playwright`.
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Dict, Iterable, List, Optional, Tuple

__all__ = ["FinEnvClient", "FinEnvError", "Cookie"]


class FinEnvError(Exception):
    """Raised when the FinEnv API returns a non-2xx response."""

    def __init__(self, status: int, code: str, message: str) -> None:
        super().__init__(message)
        self.status = status
        self.code = code
        self.message = message


Cookie = Dict[str, Any]


class FinEnvClient:
    """Synchronous client for the local FinEnv REST API.

    Example::

        client = FinEnvClient.from_env()
        runtime = client.open_environment(env_id, url="https://example.test")
        print(runtime["debugPort"])
        client.save_screenshot(env_id, "shot.png")
        client.close_environment(env_id)
    """

    def __init__(
        self,
        api_key: str,
        base_url: str = "http://127.0.0.1:17321/api/v1",
        timeout: float = 30.0,
    ) -> None:
        if not api_key:
            raise FinEnvError(0, "missing_api_key", "API Key 不能为空")
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    @classmethod
    def from_env(cls) -> "FinEnvClient":
        key = os.environ.get("FINENV_API_KEY", "")
        base = os.environ.get("FINENV_BASE_URL", "http://127.0.0.1:17321/api/v1")
        return cls(api_key=key, base_url=base)

    def __enter__(self) -> "FinEnvClient":
        return self

    def __exit__(self, *_exc: Any) -> None:
        return None

    # ------------------------------------------------------------------ HTTP
    def _url(self, path: str, query: Optional[Dict[str, Any]] = None) -> str:
        url = self.base_url + (path if path.startswith("/") else "/" + path)
        if query:
            filtered = {k: v for k, v in query.items() if v is not None and v != ""}
            if filtered:
                url += "?" + urllib.parse.urlencode(filtered)
        return url

    def _request(
        self,
        method: str,
        path: str,
        body: Optional[Any] = None,
        query: Optional[Dict[str, Any]] = None,
        headers: Optional[Dict[str, str]] = None,
        raw: bool = False,
    ) -> Any:
        data = json.dumps(body, ensure_ascii=False).encode("utf-8") if body is not None else None
        request_headers = {
            "Authorization": "Bearer " + self.api_key,
            "Accept": "*/*" if raw else "application/json",
        }
        if data is not None:
            request_headers["Content-Type"] = "application/json"
        if headers:
            request_headers.update(headers)
        request = urllib.request.Request(
            self._url(path, query),
            data=data,
            method=method,
            headers=request_headers,
        )
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                payload = response.read()
                if raw:
                    return payload
                if not payload:
                    return None
                return json.loads(payload.decode("utf-8"))
        except urllib.error.HTTPError as exc:
            payload = exc.read()
            code = "http_error"
            message = "HTTP %s" % exc.code
            try:
                parsed = json.loads(payload.decode("utf-8"))
                error = parsed.get("error", {})
                code = error.get("code", code)
                message = error.get("message", message)
            except Exception:
                if payload:
                    message = payload.decode("utf-8", errors="replace")
            raise FinEnvError(exc.code, code, message) from exc
        except urllib.error.URLError as exc:
            raise FinEnvError(0, "connection_error", "无法连接 FinEnv API: %s" % exc.reason) from exc

    # -------------------------------------------------------------- metadata
    def health(self) -> Dict[str, Any]:
        return self._request("GET", "/health")

    def stats(self) -> Dict[str, Any]:
        return self._request("GET", "/stats")

    # ---------------------------------------------------------- environments
    def list_environments(
        self,
        search: Optional[str] = None,
        group_id: Optional[str] = None,
        status: Optional[str] = None,
        project: Optional[str] = None,
        test_env: Optional[str] = None,
        tag: Optional[str] = None,
        running_only: Optional[bool] = None,
    ) -> List[Dict[str, Any]]:
        query = {
            "search": search,
            "groupId": group_id,
            "status": status,
            "project": project,
            "testEnv": test_env,
            "tag": tag,
            "runningOnly": str(running_only).lower() if running_only is not None else None,
        }
        return self._request("GET", "/envs", query=query)

    def get_environment(self, env_id: str) -> Dict[str, Any]:
        return self._request("GET", "/envs/" + urllib.parse.quote(env_id))

    def create_environment(self, **fields: Any) -> Dict[str, Any]:
        return self._request("POST", "/envs", body=fields)

    def update_environment(self, env_id: str, **fields: Any) -> Dict[str, Any]:
        return self._request("PATCH", "/envs/" + urllib.parse.quote(env_id), body=fields)

    def delete_environment(self, env_id: str) -> Dict[str, Any]:
        return self._request(
            "DELETE",
            "/envs/" + urllib.parse.quote(env_id),
            headers={"X-FinEnv-Confirm": "true"},
        )

    # -------------------------------------------------------------- browser
    def open_environment(
        self,
        env_id: str,
        url: Optional[str] = None,
        pre_script: Optional[str] = None,
        headless: Optional[bool] = None,
        wait_until: Optional[str] = None,
    ) -> Dict[str, Any]:
        payload = {
            "url": url,
            "preScript": pre_script,
            "headless": headless,
            "waitUntil": wait_until,
        }
        return self._request("POST", "/envs/%s/open" % urllib.parse.quote(env_id), body=payload)

    def close_environment(self, env_id: str) -> Dict[str, Any]:
        return self._request("POST", "/envs/%s/close" % urllib.parse.quote(env_id))

    def environment_status(self, env_id: str) -> Dict[str, Any]:
        return self._request("GET", "/envs/%s/status" % urllib.parse.quote(env_id))

    def screenshot(self, env_id: str) -> bytes:
        return self._request("GET", "/envs/%s/screenshot" % urllib.parse.quote(env_id), raw=True)

    def screenshot_info(self, env_id: str) -> Dict[str, Any]:
        return self._request(
            "GET",
            "/envs/%s/screenshot" % urllib.parse.quote(env_id),
            query={"format": "json"},
        )

    def save_screenshot(self, env_id: str, path: str) -> str:
        data = self.screenshot(env_id)
        with open(path, "wb") as handle:
            handle.write(data)
        return path

    def get_cookies(self, env_id: str) -> Dict[str, Any]:
        return self._request("GET", "/envs/%s/cookies" % urllib.parse.quote(env_id))

    def set_cookies(self, env_id: str, cookies: Iterable[Cookie]) -> Dict[str, Any]:
        return self._request(
            "PUT",
            "/envs/%s/cookies" % urllib.parse.quote(env_id),
            body={"cookies": list(cookies)},
        )

    def update_fingerprint(self, env_id: str, fingerprint: Dict[str, Any]) -> Dict[str, Any]:
        return self._request(
            "PUT",
            "/envs/%s/fingerprint" % urllib.parse.quote(env_id),
            body=fingerprint,
        )

    def detect_fingerprint(self, env_id: str) -> Dict[str, Any]:
        return self._request("POST", "/envs/%s/detect" % urllib.parse.quote(env_id))

    def create_snapshot(self, env_id: str, name: Optional[str] = None) -> Dict[str, Any]:
        return self._request(
            "POST",
            "/envs/%s/snapshot" % urllib.parse.quote(env_id),
            body={"name": name},
        )

    def list_snapshots(self, env_id: str) -> List[Dict[str, Any]]:
        return self._request("GET", "/envs/%s/snapshots" % urllib.parse.quote(env_id))

    def restore_snapshot(self, env_id: str, snapshot_id: str) -> Dict[str, Any]:
        return self._request(
            "POST",
            "/envs/%s/restore-snapshot" % urllib.parse.quote(env_id),
            body={"snapshotId": snapshot_id},
        )

    def cdp_info(self, env_id: str) -> Dict[str, Any]:
        return self._request("GET", "/envs/%s/cdp" % urllib.parse.quote(env_id))

    def navigate(self, env_id: str, url: str) -> Dict[str, Any]:
        return self._request(
            "POST",
            "/envs/%s/navigate" % urllib.parse.quote(env_id),
            body={"url": url},
        )

    def execute(self, env_id: str, script: str) -> Dict[str, Any]:
        return self._request(
            "POST",
            "/envs/%s/execute" % urllib.parse.quote(env_id),
            body={"script": script},
        )

    def start_recording(self, env_id: str, interval_ms: int = 1000) -> Dict[str, Any]:
        return self._request(
            "POST",
            "/envs/%s/recording/start" % urllib.parse.quote(env_id),
            body={"intervalMs": interval_ms},
        )

    def stop_recording(self, env_id: str) -> Dict[str, Any]:
        return self._request("POST", "/envs/%s/recording/stop" % urllib.parse.quote(env_id))

    def start_har(self, env_id: str) -> Dict[str, Any]:
        return self._request("POST", "/envs/%s/har" % urllib.parse.quote(env_id))

    def export_har(self, env_id: str) -> Dict[str, Any]:
        return self._request("GET", "/envs/%s/har" % urllib.parse.quote(env_id))

    def set_environment_proxy(
        self, env_id: str, proxy_id: Optional[str], restart: bool = False
    ) -> Dict[str, Any]:
        return self._request(
            "POST",
            "/envs/%s/proxy" % urllib.parse.quote(env_id),
            body={"proxyId": proxy_id, "restart": restart},
        )

    def align_proxy(self, env_id: str) -> Dict[str, Any]:
        return self._request("POST", "/envs/%s/align-proxy" % urllib.parse.quote(env_id))

    def get_hosts(self, env_id: str) -> List[Dict[str, Any]]:
        return self._request("GET", "/envs/%s/hosts" % urllib.parse.quote(env_id))

    def set_hosts(self, env_id: str, entries: Iterable[Dict[str, Any]]) -> List[Dict[str, Any]]:
        return self._request(
            "PUT",
            "/envs/%s/hosts" % urllib.parse.quote(env_id),
            body={"entries": list(entries)},
        )

    def get_mocks(self, env_id: str) -> List[Dict[str, Any]]:
        return self._request("GET", "/envs/%s/mocks" % urllib.parse.quote(env_id))

    def set_mocks(self, env_id: str, rules: Iterable[Dict[str, Any]]) -> List[Dict[str, Any]]:
        return self._request(
            "PUT",
            "/envs/%s/mocks" % urllib.parse.quote(env_id),
            body={"rules": list(rules)},
        )

    # --------------------------------------------------------------- proxies
    def list_proxies(self) -> List[Dict[str, Any]]:
        return self._request("GET", "/proxies")

    def create_proxy(self, **fields: Any) -> Dict[str, Any]:
        return self._request("POST", "/proxies", body=fields)

    def update_proxy(self, proxy_id: str, **fields: Any) -> Dict[str, Any]:
        return self._request("PATCH", "/proxies/" + urllib.parse.quote(proxy_id), body=fields)

    def delete_proxy(self, proxy_id: str) -> Dict[str, Any]:
        return self._request(
            "DELETE",
            "/proxies/" + urllib.parse.quote(proxy_id),
            headers={"X-FinEnv-Confirm": "true"},
        )

    def check_proxy(self, proxy_id: str) -> Dict[str, Any]:
        return self._request("POST", "/proxies/%s/check" % urllib.parse.quote(proxy_id))

    # ----------------------------------------------------------------- audit
    def list_audit(self, **query: Any) -> List[Dict[str, Any]]:
        return self._request("GET", "/audit-logs", query=query)

    def verify_audit(self) -> Dict[str, Any]:
        return self._request("GET", "/audit-logs/verify")

    # ------------------------------------------------------------ Playwright
    def connect_playwright(self, env_id: str) -> Tuple[Any, Any]:
        """Open the environment and connect Playwright (sync API).

        Returns ``(playwright, browser)``. Call ``browser.close()`` and
        ``playwright.stop()`` when finished. Requires ``pip install playwright``.
        """
        try:
            from playwright.sync_api import sync_playwright  # type: ignore
        except ImportError as exc:
            raise FinEnvError(
                0,
                "playwright_missing",
                "未安装 Playwright，请执行 pip install playwright",
            ) from exc
        self.open_environment(env_id)
        cdp = self.cdp_info(env_id)
        playwright = sync_playwright().start()
        browser = playwright.chromium.connect_over_cdp(cdp["httpEndpoint"])
        return playwright, browser
