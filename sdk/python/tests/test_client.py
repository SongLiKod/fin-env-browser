import io
import json
import os
import sys
import unittest
import urllib.error
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from finenv import FinEnvClient, FinEnvError


class ClientTests(unittest.TestCase):
    def setUp(self):
        self.client = FinEnvClient(api_key="finenv_test", base_url="http://127.0.0.1:17321/api/v1/")

    def test_query_building_skips_empty_values(self):
        url = self.client._url(
            "/envs",
            {
                "search": "订单",
                "runningOnly": "true",
                "empty": "",
                "none": None,
            },
        )
        self.assertIn("/api/v1/envs?", url)
        self.assertIn("search=%E8%AE%A2%E5%8D%95", url)
        self.assertIn("runningOnly=true", url)
        self.assertNotIn("empty", url)
        self.assertNotIn("none", url)

    def test_authorization_header_and_json_body(self):
        captured = {}

        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *args):
                return None

            def read(self):
                return json.dumps({"id": "env-1"}).encode()

        def fake_urlopen(request, timeout):
            captured["url"] = request.full_url
            captured["method"] = request.get_method()
            captured["headers"] = dict(request.header_items())
            captured["body"] = request.data
            captured["timeout"] = timeout
            return Response()

        with mock.patch("urllib.request.urlopen", fake_urlopen):
            result = self.client.create_environment(name="CI 环境", templateOs="win11")

        self.assertEqual(result["id"], "env-1")
        self.assertEqual(captured["method"], "POST")
        self.assertEqual(captured["headers"].get("Authorization"), "Bearer finenv_test")
        self.assertIn(b"CI", captured["body"])

    def test_error_payload_is_exposed(self):
        payload = json.dumps({"error": {"code": "forbidden", "message": "无权限"}}).encode()

        def fake_urlopen(request, timeout):
            raise urllib.error.HTTPError(request.full_url, 403, "Forbidden", {}, io.BytesIO(payload))

        with mock.patch("urllib.request.urlopen", fake_urlopen):
            with self.assertRaises(FinEnvError) as context:
                self.client.list_environments()
        self.assertEqual(context.exception.status, 403)
        self.assertEqual(context.exception.code, "forbidden")
        self.assertEqual(context.exception.message, "无权限")

    def test_from_env_requires_key(self):
        with mock.patch.dict(os.environ, {"FINENV_API_KEY": ""}, clear=False):
            with self.assertRaises(FinEnvError):
                FinEnvClient.from_env()
        with mock.patch.dict(
            os.environ,
            {"FINENV_API_KEY": "finenv_env", "FINENV_BASE_URL": "http://localhost:1/api/v1"},
            clear=False,
        ):
            client = FinEnvClient.from_env()
            self.assertEqual(client.api_key, "finenv_env")
            self.assertEqual(client.base_url, "http://localhost:1/api/v1")


if __name__ == "__main__":
    unittest.main()
