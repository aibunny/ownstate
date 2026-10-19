import base64
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
import os
import secrets
import tempfile
import threading
import unittest
import urllib.error
import urllib.parse
import urllib.request
from http.server import ThreadingHTTPServer
from pathlib import Path


class GatewayTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        os.environ.update({
            "PUBLIC_URL": "http://127.0.0.1:8787",
            "OWNER_EMAIL": "owner@example.com",
            "DB_PATH": str(Path(cls.tmp.name) / "test.sqlite3"),
            "DEFAULT_NAMESPACE": "personal",
        })
        spec = importlib.util.spec_from_file_location("gateway", Path(__file__).with_name("server.py"))
        cls.gateway = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.gateway)
        cls.gateway.init_db()
        cls.mail_links = []
        cls.gateway.send_email = lambda url, name: cls.mail_links.append(url)
        cls.gateway.api_post = lambda path, body: {"path": path, "body": body}
        cls.gateway.api_get = lambda path: {"namespaces": [{"namespace": "repo_demo", "description": "Demo repo", "documentCount": 2, "memoryCount": 1, "system": {"createdAt": "2026-01-01"}}]}
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), cls.gateway.Handler)
        cls.base = f"http://127.0.0.1:{cls.server.server_port}"
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.tmp.cleanup()

    def request(self, path, data=None, headers=None, method=None):
        req = urllib.request.Request(self.base + path, data=data, headers=headers or {}, method=method)
        class NoRedirect(urllib.request.HTTPRedirectHandler):
            def redirect_request(self, request, fp, code, msg, headers, newurl):
                return None
        opener = urllib.request.build_opener(NoRedirect)
        try:
            with opener.open(req, timeout=5) as response:
                return response.status, dict(response.headers), response.read()
        except urllib.error.HTTPError as exc:
            try:
                return exc.code, dict(exc.headers), exc.read()
            finally:
                exc.close()

    def json_post(self, path, value, headers=None):
        return self.request(path, json.dumps(value).encode(), {"Content-Type": "application/json", **(headers or {})})

    def form_post(self, path, value):
        return self.request(path, urllib.parse.urlencode(value).encode(), {"Content-Type": "application/x-www-form-urlencoded"})

    def test_oauth_and_mcp(self):
        status, _, body = self.request("/.well-known/oauth-protected-resource/mcp")
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body)["resource"], self.gateway.RESOURCE)

        status, _, body = self.json_post("/register", {"client_name": "Test chat", "redirect_uris": ["https://chat.example/callback"], "token_endpoint_auth_method": "none"})
        self.assertEqual(status, 201)
        client = json.loads(body)["client_id"]

        verifier = secrets.token_urlsafe(40)
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
        query = urllib.parse.urlencode({"response_type": "code", "client_id": client, "redirect_uri": "https://chat.example/callback", "code_challenge": challenge, "code_challenge_method": "S256", "state": "sample-state", "resource": self.gateway.RESOURCE})
        status, _, body = self.request("/authorize?" + query)
        self.assertEqual(status, 200)
        self.assertIn(b"Test chat", body)
        with self.gateway.db() as con:
            request_id = con.execute("SELECT id FROM requests ORDER BY created_at DESC LIMIT 1").fetchone()[0]
        status, _, body = self.form_post("/authorize/send", {"request_id": request_id})
        self.assertEqual(status, 200)
        self.assertIn(b"/authorize/status", body)
        self.assertIn(b"window.location.replace", body)
        self.assertTrue(self.mail_links)
        status, _, body = self.request("/authorize/status?" + urllib.parse.urlencode({"request_id": request_id}))
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body)["status"], "pending")
        ticket = urllib.parse.urlsplit(self.mail_links[-1]).query
        status, _, body = self.request("/login/verify?" + ticket)
        self.assertEqual(status, 200)
        self.assertIn(b"Allow connection", body)
        status, _, body = self.form_post("/login/verify", {"ticket": urllib.parse.parse_qs(ticket)["ticket"][0]})
        self.assertEqual(status, 200)
        self.assertIn(b"Connection approved", body)
        status, _, body = self.request("/authorize/status?" + urllib.parse.urlencode({"request_id": request_id}))
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body)["status"], "approved")
        callback = urllib.parse.urlsplit(json.loads(body)["redirect"])
        values = urllib.parse.parse_qs(callback.query)
        self.assertEqual(values["state"], ["sample-state"])
        code = values["code"][0]
        status, _, body = self.request("/authorize/status?" + urllib.parse.urlencode({"request_id": request_id}))
        self.assertEqual(status, 410)
        self.assertEqual(json.loads(body)["status"], "expired")
        status, _, _ = self.form_post("/login/verify", {"ticket": urllib.parse.parse_qs(ticket)["ticket"][0]})
        self.assertEqual(status, 400)

        bad = {"grant_type": "authorization_code", "client_id": client, "code": code, "redirect_uri": "https://chat.example/callback", "code_verifier": "x" * 44}
        status, _, _ = self.form_post("/token", bad)
        self.assertEqual(status, 400)
        good = {**bad, "code_verifier": verifier, "resource": self.gateway.RESOURCE}
        status, _, body = self.form_post("/token", good)
        self.assertEqual(status, 200)
        tokens = json.loads(body)
        status, _, _ = self.form_post("/token", good)
        self.assertEqual(status, 400)

        call = {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "who_am_i", "arguments": {}}}
        status, _, body = self.json_post("/mcp", call)
        self.assertEqual(status, 200)
        self.assertTrue(json.loads(body)["result"]["isError"])
        self.assertIn("mcp/www_authenticate", json.loads(body)["result"]["_meta"])
        status, _, body = self.json_post("/mcp", call, {"Authorization": "Bearer " + tokens["access_token"]})
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body)["result"]["structuredContent"]["email"], "owner@example.com")
        call["params"]["name"] = "list_namespaces"
        status, _, body = self.json_post("/mcp", call, {"Authorization": "Bearer " + tokens["access_token"]})
        self.assertEqual(status, 200)
        spaces = json.loads(body)["result"]["structuredContent"]["namespaces"]
        self.assertEqual(spaces[0]["namespace"], "repo_demo")
        self.assertNotIn("system", spaces[0])
        call["params"]["name"] = "get_profile"
        status, _, body = self.json_post("/mcp", call, {"Authorization": "Bearer " + tokens["access_token"]})
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(body)["result"]["structuredContent"]["body"], {})

        status, _, body = self.form_post("/token", {"grant_type": "refresh_token", "client_id": client, "refresh_token": tokens["refresh_token"], "resource": self.gateway.RESOURCE})
        self.assertEqual(status, 200)
        renewed = json.loads(body)
        self.assertNotEqual(renewed["refresh_token"], tokens["refresh_token"])
        status, _, _ = self.form_post("/token", {"grant_type": "refresh_token", "client_id": client, "refresh_token": tokens["refresh_token"]})
        self.assertEqual(status, 400)
        refresh_form = {"grant_type": "refresh_token", "client_id": client, "refresh_token": renewed["refresh_token"]}
        with ThreadPoolExecutor(max_workers=2) as pool:
            statuses = list(pool.map(lambda _: self.form_post("/token", refresh_form)[0], range(2)))
        self.assertEqual(sorted(statuses), [200, 400])

    def test_rejects_unregistered_redirect(self):
        status, _, _ = self.json_post("/register", {"redirect_uris": ["http://evil.example/callback"]})
        self.assertEqual(status, 400)


if __name__ == "__main__":
    unittest.main()
