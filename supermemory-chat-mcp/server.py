"""Single-owner OAuth 2.1 and MCP gateway for a self-hosted Supermemory API."""

from __future__ import annotations

import base64
import hashlib
import html
import json
import os
import re
import secrets
import sqlite3
import time
import urllib.error
import urllib.parse
import urllib.request
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


PUBLIC_URL = os.environ.get("PUBLIC_URL", "http://127.0.0.1:8787").rstrip("/")
API_BASE = os.environ.get("SUPERMEMORY_API_BASE", "").rstrip("/")
API_KEY = os.environ.get("SUPERMEMORY_API_KEY", "")
OWNER_EMAIL = os.environ.get("OWNER_EMAIL", "").strip().lower()
RESEND_KEY = os.environ.get("RESEND_API_KEY", "")
FROM_EMAIL = os.environ.get("FROM_EMAIL", "")
DB_PATH = os.environ.get("DB_PATH", "/data/gateway.sqlite3")
DEFAULT_NAMESPACE = os.environ.get("DEFAULT_NAMESPACE", "personal")
RESOURCE = f"{PUBLIC_URL}/mcp"
SCOPE = "memory.read memory.write"
MAX_BODY = 256_000
NAME_RE = re.compile(r"^[a-zA-Z0-9_.:-]{1,128}$")


def digest(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def random_token() -> str:
    return secrets.token_urlsafe(32)


@contextmanager
def db():
    con = sqlite3.connect(DB_PATH, timeout=15)
    con.row_factory = sqlite3.Row
    try:
        with con:
            yield con
    finally:
        con.close()


def init_db() -> None:
    Path(DB_PATH).parent.mkdir(parents=True, exist_ok=True)
    with db() as con:
        con.executescript("""
        CREATE TABLE IF NOT EXISTS clients (
          client_id TEXT PRIMARY KEY,
          name TEXT NOT NULL,
          redirects TEXT NOT NULL,
          created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS requests (
          id TEXT PRIMARY KEY,
          client_id TEXT NOT NULL,
          redirect_uri TEXT NOT NULL,
          state TEXT,
          challenge TEXT NOT NULL,
          scope TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          mail_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS tickets (
          token_hash TEXT PRIMARY KEY,
          request_id TEXT NOT NULL,
          expires_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS codes (
          code_hash TEXT PRIMARY KEY,
          client_id TEXT NOT NULL,
          redirect_uri TEXT NOT NULL,
          challenge TEXT NOT NULL,
          scope TEXT NOT NULL,
          expires_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tokens (
          token_hash TEXT PRIMARY KEY,
          client_id TEXT NOT NULL,
          kind TEXT NOT NULL,
          scope TEXT NOT NULL,
          expires_at INTEGER,
          created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS mail_events (created_at INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS registration_events (created_at INTEGER NOT NULL);
        """)
        now = int(time.time())
        con.execute("DELETE FROM requests WHERE created_at<?", (now - 3600,))
        con.execute("DELETE FROM tickets WHERE expires_at<?", (now,))
        con.execute("DELETE FROM codes WHERE expires_at<?", (now,))
        con.execute("DELETE FROM tokens WHERE kind='access' AND expires_at<?", (now,))
        con.execute("DELETE FROM mail_events WHERE created_at<?", (now - 3600,))
        con.execute("DELETE FROM registration_events WHERE created_at<?", (now - 3600,))


def parse_scope(raw: str) -> str | None:
    values = set(raw.split()) if raw else set(SCOPE.split())
    return " ".join(sorted(values)) if values and values <= set(SCOPE.split()) else None


def valid_redirect(uri: str) -> bool:
    try:
        p = urllib.parse.urlsplit(uri)
        return p.scheme == "https" and bool(p.hostname) and not p.username and not p.password and not p.fragment
    except ValueError:
        return False


def valid_namespace(value: object) -> str | None:
    if value is None or value == "":
        return DEFAULT_NAMESPACE
    if isinstance(value, str) and NAME_RE.fullmatch(value):
        return value
    return None


def api_post(path: str, body: dict) -> dict:
    if not API_BASE or not API_KEY:
        raise RuntimeError("Supermemory API is not configured")
    req = urllib.request.Request(
        API_BASE + path,
        data=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=25) as res:
            return json.load(res)
    except urllib.error.HTTPError as exc:
        raise RuntimeError(f"Supermemory API returned HTTP {exc.code}") from None


def api_get(path: str) -> dict:
    if not API_BASE or not API_KEY:
        raise RuntimeError("Supermemory API is not configured")
    req = urllib.request.Request(API_BASE + path, headers={"Authorization": f"Bearer {API_KEY}"})
    try:
        with urllib.request.urlopen(req, timeout=12) as res:
            return json.load(res)
    except urllib.error.HTTPError as exc:
        raise RuntimeError(f"Supermemory API returned HTTP {exc.code}") from None


def send_email(url: str, client_name: str) -> None:
    if not RESEND_KEY or not FROM_EMAIL:
        raise RuntimeError("Email delivery is not configured")
    payload = {
        "from": FROM_EMAIL,
        "to": [OWNER_EMAIL],
        "subject": "Connect Supermemory to your chat",
        "html": (
            "<p>A chat connector named <strong>" + html.escape(client_name) + "</strong> "
            "requested access to your Supermemory memory.</p>"
            "<p><a href=\"" + html.escape(url, quote=True) + "\">Allow this connection</a></p>"
            "<p>This link expires in 10 minutes. If you did not request it, ignore this email.</p>"
        ),
    }
    req = urllib.request.Request(
        "https://api.resend.com/emails",
        data=json.dumps(payload).encode(),
        headers={"Authorization": f"Bearer {RESEND_KEY}", "Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=12) as res:
            if res.status not in (200, 201):
                raise RuntimeError("Email delivery failed")
    except urllib.error.HTTPError as exc:
        raise RuntimeError(f"Email delivery returned HTTP {exc.code}") from None


def tool_definitions() -> list[dict]:
    auth = [{"type": "oauth2", "scopes": SCOPE.split()}]
    ns = {"type": "string", "description": "Optional exact Supermemory namespace. Defaults to your personal chat namespace. Supply a repository namespace only when explicitly working with that repository."}
    return [
        {"name": "who_am_i", "title": "Supermemory account", "description": "Show the connected Supermemory account and default chat namespace.", "inputSchema": {"type": "object", "properties": {}}, "outputSchema": {"type": "object", "properties": {"id": {"type": "string"}, "name": {"type": "string"}, "email": {"type": "string"}, "nickname": {"type": "string"}}, "required": ["id"], "additionalProperties": False}, "annotations": {"readOnlyHint": True}, "securitySchemes": auth, "_meta": {"openai/profile": True}},
        {"name": "search_memory", "title": "Search Supermemory", "description": "Recall relevant saved memories. Search the personal chat namespace by default; use a repository namespace only when the user names that project.", "inputSchema": {"type": "object", "properties": {"query": {"type": "string", "minLength": 1}, "namespace": ns}, "required": ["query"]}, "annotations": {"readOnlyHint": True}, "securitySchemes": auth},
        {"name": "get_profile", "title": "Get Supermemory profile", "description": "Read stable and recent memory context from the chosen namespace.", "inputSchema": {"type": "object", "properties": {"namespace": ns}}, "annotations": {"readOnlyHint": True}, "securitySchemes": auth},
        {"name": "list_namespaces", "title": "List Supermemory spaces", "description": "List available memory namespaces so you can select a named repository when the user asks about it. Do not combine unrelated repository memories.", "inputSchema": {"type": "object", "properties": {}}, "annotations": {"readOnlyHint": True}, "securitySchemes": auth},
        {"name": "add_memory", "title": "Save to Supermemory", "description": "Save a verified decision, preference, or durable fact. Never save secrets, credentials, or an unreviewed full chat transcript.", "inputSchema": {"type": "object", "properties": {"content": {"type": "string", "minLength": 1, "maxLength": 10000}, "namespace": ns}, "required": ["content"]}, "annotations": {"readOnlyHint": False, "destructiveHint": False}, "securitySchemes": auth},
    ]


def call_tool(name: str, args: dict) -> dict:
    namespace = valid_namespace(args.get("namespace"))
    if namespace is None:
        raise ValueError("Invalid namespace")
    if name == "who_am_i":
        return {"id": digest(OWNER_EMAIL)[:24], "name": OWNER_EMAIL.split("@")[0], "email": OWNER_EMAIL, "nickname": f"Supermemory · {namespace}"}
    if name == "list_namespaces":
        raw = api_get("/namespaces")
        return {"namespaces": [{"namespace": item.get("namespace"), "description": item.get("description"), "documentCount": item.get("documentCount"), "memoryCount": item.get("memoryCount")} for item in raw.get("namespaces", []) if isinstance(item, dict)]}
    path = "/ns/" + urllib.parse.quote(namespace, safe="")
    if name == "search_memory":
        query = args.get("query")
        if not isinstance(query, str) or not 0 < len(query.strip()) <= 500:
            raise ValueError("Query must be 1–500 characters")
        return api_post(path + "/search", {"query": query.strip()})
    if name == "get_profile":
        return api_post(path + "/profile", {"q": ""})
    if name == "add_memory":
        content = args.get("content")
        if not isinstance(content, str) or not 0 < len(content.strip()) <= 10000:
            raise ValueError("Content must be 1–10000 characters")
        return api_post(path + "/document", {"content": content.strip(), "id": "chat-" + secrets.token_hex(12), "metadata": {"source": "chat-mcp"}})
    raise ValueError("Unknown tool")


class Handler(BaseHTTPRequestHandler):
    server_version = "SupermemoryChatMCP/1"

    def log_message(self, fmt: str, *args: object) -> None:
        # Never log email tickets, OAuth codes, bearer tokens, or query strings.
        print(f"{self.address_string()} {self.command} {urllib.parse.urlsplit(self.path).path} {args[1] if len(args)>1 else ''}", flush=True)

    def send(self, status: int, payload: object, *, content_type: str = "application/json", extra: dict | None = None) -> None:
        data = (json.dumps(payload) if content_type == "application/json" else str(payload)).encode()
        self.send_response(status)
        self.send_header("Content-Type", content_type + "; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("X-Content-Type-Options", "nosniff")
        for key, value in (extra or {}).items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(data)

    def redirect(self, location: str) -> None:
        self.send_response(302)
        self.send_header("Location", location)
        self.send_header("Cache-Control", "no-store")
        self.send_header("Referrer-Policy", "no-referrer")
        self.end_headers()

    def body(self) -> bytes | None:
        try:
            size = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            return None
        if size < 0 or size > MAX_BODY:
            return None
        return self.rfile.read(size)

    def json_body(self) -> dict | None:
        try:
            value = json.loads(self.body() or b"")
            return value if isinstance(value, dict) else None
        except (ValueError, TypeError):
            return None

    def form(self) -> dict:
        try:
            return {k: v[-1] for k, v in urllib.parse.parse_qs((self.body() or b"").decode(), keep_blank_values=True).items()}
        except UnicodeDecodeError:
            return {}

    def do_GET(self) -> None:
        p = urllib.parse.urlsplit(self.path)
        if p.path == "/health":
            return self.send(200, {"ok": True})
        if p.path in ("/.well-known/oauth-protected-resource", "/.well-known/oauth-protected-resource/mcp"):
            return self.send(200, {"resource": RESOURCE, "authorization_servers": [PUBLIC_URL], "scopes_supported": SCOPE.split(), "bearer_methods_supported": ["header"]})
        if p.path in ("/.well-known/oauth-authorization-server", "/.well-known/openid-configuration"):
            return self.send(200, {"issuer": PUBLIC_URL, "authorization_endpoint": PUBLIC_URL + "/authorize", "token_endpoint": PUBLIC_URL + "/token", "registration_endpoint": PUBLIC_URL + "/register", "revocation_endpoint": PUBLIC_URL + "/revoke", "response_types_supported": ["code"], "grant_types_supported": ["authorization_code", "refresh_token"], "code_challenge_methods_supported": ["S256"], "token_endpoint_auth_methods_supported": ["none"], "scopes_supported": SCOPE.split(), "service_documentation": PUBLIC_URL + "/"})
        if p.path == "/authorize":
            return self.authorize(urllib.parse.parse_qs(p.query))
        if p.path == "/login/verify":
            return self.show_login(urllib.parse.parse_qs(p.query))
        if p.path == "/mcp":
            return self.send(405, {"error": "Use MCP streamable HTTP POST"}, extra={"Allow": "POST"})
        if p.path == "/":
            return self.send(200, "<h1>Supermemory chat connector</h1><p>Use the /mcp URL in Claude or ChatGPT. Sign-in is limited to the configured owner email.</p>", content_type="text/html")
        self.send(404, {"error": "Not found"})

    def do_POST(self) -> None:
        p = urllib.parse.urlsplit(self.path).path
        if p == "/register":
            return self.register()
        if p == "/authorize/send":
            return self.send_login()
        if p == "/login/verify":
            return self.verify_login(self.form().get("ticket", ""))
        if p == "/token":
            return self.token()
        if p == "/revoke":
            return self.revoke()
        if p == "/mcp":
            return self.mcp()
        self.send(404, {"error": "Not found"})

    def register(self) -> None:
        value = self.json_body()
        redirects = value.get("redirect_uris") if value else None
        if not isinstance(redirects, list) or not 1 <= len(redirects) <= 5 or not all(isinstance(u, str) and valid_redirect(u) for u in redirects):
            return self.send(400, {"error": "invalid_redirect_uri"})
        if value.get("token_endpoint_auth_method", "none") != "none":
            return self.send(400, {"error": "invalid_client_metadata"})
        name = str(value.get("client_name") or urllib.parse.urlsplit(redirects[0]).hostname)[:100]
        client_id = "smc_" + random_token()
        with db() as con:
            con.execute("BEGIN IMMEDIATE")
            now = int(time.time())
            recent = con.execute("SELECT COUNT(*) FROM registration_events WHERE created_at>?", (now - 600,)).fetchone()[0]
            if recent >= 30:
                return self.send(429, {"error": "rate_limited"})
            con.execute("INSERT INTO clients VALUES (?,?,?,?)", (client_id, name, json.dumps(redirects), int(time.time())))
            con.execute("INSERT INTO registration_events VALUES (?)", (now,))
        self.send(201, {"client_id": client_id, "client_name": name, "redirect_uris": redirects, "token_endpoint_auth_method": "none", "grant_types": ["authorization_code", "refresh_token"], "response_types": ["code"]})

    def authorize(self, q: dict) -> None:
        get = lambda k: q.get(k, [""])[-1]
        client_id, redirect, challenge = get("client_id"), get("redirect_uri"), get("code_challenge")
        scope = parse_scope(get("scope"))
        with db() as con:
            client = con.execute("SELECT * FROM clients WHERE client_id=?", (client_id,)).fetchone()
        if not client or redirect not in json.loads(client["redirects"]) or get("response_type") != "code" or get("code_challenge_method") != "S256" or not re.fullmatch(r"[A-Za-z0-9_-]{43,128}", challenge) or get("resource") not in ("", RESOURCE) or scope is None:
            return self.send(400, "Invalid authorization request.", content_type="text/plain")
        request_id = random_token()
        with db() as con:
            con.execute("INSERT INTO requests (id,client_id,redirect_uri,state,challenge,scope,created_at) VALUES (?,?,?,?,?,?,?)", (request_id, client_id, redirect, get("state"), challenge, scope, int(time.time())))
        page = f"<html><head><meta name='referrer' content='no-referrer'><title>Connect Supermemory</title></head><body><main style='font:16px system-ui;max-width:32rem;margin:8vh auto;padding:2rem'><h1>Connect Supermemory</h1><p><strong>{html.escape(client['name'])}</strong> ({html.escape(urllib.parse.urlsplit(redirect).hostname or '')}) requests read and write access to your memories.</p><p>A one-time sign-in link will be sent to <strong>{html.escape(OWNER_EMAIL)}</strong>.</p><form method='post' action='/authorize/send'><input type='hidden' name='request_id' value='{request_id}'><button style='padding:.8rem 1.2rem'>Email sign-in link</button></form></main></body></html>"
        self.send(200, page, content_type="text/html")

    def send_login(self) -> None:
        request_id = self.form().get("request_id", "")
        now = int(time.time())
        with db() as con:
            con.execute("BEGIN IMMEDIATE")
            req = con.execute("SELECT requests.*,clients.name FROM requests JOIN clients USING (client_id) WHERE requests.id=?", (request_id,)).fetchone()
            sent = con.execute("SELECT COUNT(*) FROM mail_events WHERE created_at>?", (now - 600,)).fetchone()[0]
            if not req or req["created_at"] < now - 600 or req["mail_count"] >= 2 or sent >= 5:
                return self.send(429, "This request expired or reached the email limit. Start connecting again.", content_type="text/plain")
            ticket = random_token()
            con.execute("INSERT INTO tickets VALUES (?,?,?)", (digest(ticket), request_id, now + 600))
            con.execute("UPDATE requests SET mail_count=mail_count+1 WHERE id=?", (request_id,))
            con.execute("INSERT INTO mail_events VALUES (?)", (now,))
        link = PUBLIC_URL + "/login/verify?ticket=" + urllib.parse.quote(ticket)
        try:
            send_email(link, req["name"])
        except RuntimeError:
            return self.send(503, "Email delivery is unavailable. Please try again later.", content_type="text/plain")
        self.send(200, f"<html><body style='font:16px system-ui;max-width:32rem;margin:8vh auto'><h1>Check your email</h1><p>Open the sign-in link sent to {html.escape(OWNER_EMAIL)}. The link expires in 10 minutes.</p></body></html>", content_type="text/html")

    def show_login(self, q: dict) -> None:
        token = q.get("ticket", [""])[-1]
        if not token:
            return self.send(400, "Missing sign-in link", content_type="text/plain")
        with db() as con:
            row = con.execute("SELECT tickets.expires_at,clients.name,requests.redirect_uri FROM tickets JOIN requests ON tickets.request_id=requests.id JOIN clients USING (client_id) WHERE tickets.token_hash=?", (digest(token),)).fetchone()
        if not row or row["expires_at"] < int(time.time()):
            return self.send(400, "Sign-in link expired or already used", content_type="text/plain")
        page = f"<html><head><meta name='referrer' content='no-referrer'><title>Approve Supermemory connection</title></head><body><main style='font:16px system-ui;max-width:32rem;margin:8vh auto;padding:2rem'><h1>Approve Supermemory connection</h1><p><strong>{html.escape(row['name'])}</strong> ({html.escape(urllib.parse.urlsplit(row['redirect_uri']).hostname or '')}) will be able to read and save memories.</p><form method='post' action='/login/verify'><input type='hidden' name='ticket' value='{html.escape(token, quote=True)}'><button style='padding:.8rem 1.2rem'>Allow connection</button></form></main></body></html>"
        self.send(200, page, content_type="text/html")

    def verify_login(self, token: str) -> None:
        now = int(time.time())
        if not token:
            return self.send(400, "Missing sign-in link", content_type="text/plain")
        with db() as con:
            con.execute("BEGIN IMMEDIATE")
            row = con.execute("SELECT tickets.*,requests.* FROM tickets JOIN requests ON tickets.request_id=requests.id WHERE tickets.token_hash=?", (digest(token),)).fetchone()
            if not row or row["expires_at"] < now or row["created_at"] < now - 600:
                return self.send(400, "Sign-in link expired or already used", content_type="text/plain")
            con.execute("DELETE FROM tickets WHERE token_hash=?", (digest(token),))
            con.execute("DELETE FROM requests WHERE id=?", (row["request_id"],))
            code = random_token()
            con.execute("INSERT INTO codes VALUES (?,?,?,?,?,?)", (digest(code), row["client_id"], row["redirect_uri"], row["challenge"], row["scope"], now + 180))
        parts = urllib.parse.urlsplit(row["redirect_uri"])
        params = urllib.parse.parse_qsl(parts.query, keep_blank_values=True)
        params.append(("code", code))
        if row["state"]:
            params.append(("state", row["state"]))
        params.append(("iss", PUBLIC_URL))
        self.redirect(urllib.parse.urlunsplit((parts.scheme, parts.netloc, parts.path, urllib.parse.urlencode(params), parts.fragment)))

    def token(self) -> None:
        form = self.form()
        grant, client_id = form.get("grant_type"), form.get("client_id")
        now = int(time.time())
        if grant == "authorization_code":
            code, verifier = form.get("code", ""), form.get("code_verifier", "")
            if not re.fullmatch(r"[A-Za-z0-9._~-]{43,128}", verifier) or form.get("resource") not in (None, "", RESOURCE):
                return self.send(400, {"error": "invalid_grant"})
            computed = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b"=").decode()
            with db() as con:
                con.execute("BEGIN IMMEDIATE")
                row = con.execute("SELECT * FROM codes WHERE code_hash=?", (digest(code),)).fetchone()
                if not row or row["expires_at"] < now or row["client_id"] != client_id or row["redirect_uri"] != form.get("redirect_uri") or not secrets.compare_digest(row["challenge"], computed):
                    return self.send(400, {"error": "invalid_grant"})
                con.execute("DELETE FROM codes WHERE code_hash=?", (digest(code),))
                scope = row["scope"]
                access, refresh = random_token(), random_token()
                con.execute("INSERT INTO tokens VALUES (?,?,?,?,?,?)", (digest(access), client_id, "access", scope, now + 3600, now))
                con.execute("INSERT INTO tokens VALUES (?,?,?,?,?,?)", (digest(refresh), client_id, "refresh", scope, None, now))
            return self.send(200, {"access_token": access, "token_type": "Bearer", "expires_in": 3600, "refresh_token": refresh, "scope": scope})
        if grant == "refresh_token":
            old = form.get("refresh_token", "")
            with db() as con:
                con.execute("BEGIN IMMEDIATE")
                row = con.execute("SELECT * FROM tokens WHERE token_hash=? AND kind='refresh'", (digest(old),)).fetchone()
                if not row or row["client_id"] != client_id or form.get("resource") not in (None, "", RESOURCE):
                    return self.send(400, {"error": "invalid_grant"})
                con.execute("DELETE FROM tokens WHERE token_hash=?", (digest(old),))
                access, refresh = random_token(), random_token()
                con.execute("INSERT INTO tokens VALUES (?,?,?,?,?,?)", (digest(access), client_id, "access", row["scope"], now + 3600, now))
                con.execute("INSERT INTO tokens VALUES (?,?,?,?,?,?)", (digest(refresh), client_id, "refresh", row["scope"], None, now))
            return self.send(200, {"access_token": access, "token_type": "Bearer", "expires_in": 3600, "refresh_token": refresh, "scope": row["scope"]})
        self.send(400, {"error": "unsupported_grant_type"})

    def revoke(self) -> None:
        form = self.form()
        with db() as con:
            con.execute("DELETE FROM tokens WHERE token_hash=? AND client_id=?", (digest(form.get("token", "")), form.get("client_id", "")))
        self.send(200, {})

    def mcp(self) -> None:
        value = self.json_body()
        if value is None:
            return self.send(400, {"error": "Invalid JSON"})
        method, request_id = value.get("method"), value.get("id")
        if not isinstance(method, str) or value.get("jsonrpc") != "2.0":
            return self.send(400, {"error": "Invalid JSON-RPC request"})
        if request_id is None:
            self.send_response(202)
            self.send_header("Content-Length", "0")
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            return
        reply = lambda result: self.send(200, {"jsonrpc": "2.0", "id": request_id, "result": result}, extra={"MCP-Protocol-Version": "2025-03-26"})
        if method == "initialize":
            return reply({"protocolVersion": "2025-03-26", "capabilities": {"tools": {"listChanged": False}}, "serverInfo": {"name": "supermemory-chat", "version": "1.0.0"}})
        if method == "ping":
            return reply({})
        if method == "tools/list":
            return reply({"tools": tool_definitions()})
        if method != "tools/call":
            return self.send(200, {"jsonrpc": "2.0", "id": request_id, "error": {"code": -32601, "message": "Method not found"}})
        params = value.get("params") or {}
        if not isinstance(params, dict) or params.get("name") not in {t["name"] for t in tool_definitions()}:
            return self.send(200, {"jsonrpc": "2.0", "id": request_id, "error": {"code": -32602, "message": "Unknown tool"}})
        bearer = self.headers.get("Authorization", "")
        token = bearer[7:] if bearer.startswith("Bearer ") else ""
        with db() as con:
            row = con.execute("SELECT * FROM tokens WHERE token_hash=? AND kind='access'", (digest(token),)).fetchone() if token else None
        needed = "memory.write" if params["name"] == "add_memory" else "memory.read"
        if not row or row["expires_at"] <= int(time.time()) or needed not in row["scope"].split():
            challenge = f'Bearer resource_metadata="{PUBLIC_URL}/.well-known/oauth-protected-resource/mcp", error="invalid_token", error_description="Sign in to Supermemory to continue"'
            return reply({"content": [{"type": "text", "text": "Connect your Supermemory account to use this tool."}], "_meta": {"mcp/www_authenticate": [challenge]}, "isError": True})
        args = params.get("arguments") or {}
        if not isinstance(args, dict):
            return reply({"content": [{"type": "text", "text": "Invalid tool arguments"}], "isError": True})
        try:
            result = call_tool(params["name"], args)
            return reply({"content": [{"type": "text", "text": json.dumps(result, ensure_ascii=False)}], "structuredContent": result, "isError": False})
        except ValueError as exc:
            return reply({"content": [{"type": "text", "text": str(exc)}], "isError": True})
        except Exception as exc:
            print("Tool error:", type(exc).__name__, str(exc)[:120], flush=True)
            return reply({"content": [{"type": "text", "text": "Supermemory is temporarily unavailable."}], "isError": True})


if __name__ == "__main__":
    if not OWNER_EMAIL or not NAME_RE.fullmatch(DEFAULT_NAMESPACE):
        raise SystemExit("OWNER_EMAIL and a valid DEFAULT_NAMESPACE are required")
    if PUBLIC_URL.startswith("http://") and not PUBLIC_URL.startswith("http://127.0.0.1"):
        raise SystemExit("PUBLIC_URL must use HTTPS")
    init_db()
    port = int(os.environ.get("PORT", "8787"))
    print(f"Supermemory chat MCP listening on {port}", flush=True)
    ThreadingHTTPServer(("0.0.0.0", port), Handler).serve_forever()
