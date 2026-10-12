#!/usr/bin/env python3
"""Capture supported Codex/Claude Code hook events without touching transcripts.

Set OWNSTATE_CAPTURE_URL to the public API origin and
OWNSTATE_CAPTURE_TOKEN_FILE to a chmod-600 file containing one account-owned
MCP connection token. This script is intentionally silent on stdout because
both hosts interpret hook stdout as model or control output.
"""
import json
import os
import pathlib
import sys
import urllib.error
import urllib.request
import uuid


def event_from_hook(source, hook):
    session = hook.get("session_id")
    name = hook.get("hook_event_name")
    if not isinstance(session, str) or not session:
        return None
    if name == "UserPromptSubmit":
        content = hook.get("prompt")
        event_type, actor = "USER_MESSAGE", "USER"
        stable = hook.get("turn_id")
        event_id = f"{stable}:user" if stable else None
    elif name == "Stop":
        content = hook.get("last_assistant_message")
        event_type, actor = "ASSISTANT_MESSAGE", "ASSISTANT"
        stable = hook.get("turn_id")
        event_id = f"{stable}:assistant" if stable else None
    elif name == "PostToolUse":
        tool = hook.get("tool_name")
        if not isinstance(tool, str) or not tool:
            return None
        content = f"Tool completed: {tool}"
        event_type, actor = "TOOL_CALL", "TOOL"
        stable = hook.get("tool_use_id")
        event_id = f"{stable}:tool" if stable else None
    else:
        return None
    if not isinstance(content, str) or not content:
        return None
    if len(content.encode("utf-8")) > 256 * 1024:
        content = content.encode("utf-8")[:256 * 1024].decode("utf-8", errors="ignore")
    return {
        "source": source,
        "source_conversation_id": session,
        "source_event_id": event_id or str(uuid.uuid4()),
        "capture_mode": "PARTIAL_ADAPTER",
        "event_type": event_type,
        "actor_type": actor,
        "content": content,
    }


def send(url, token, event):
    request = urllib.request.Request(
        url.rstrip("/") + "/capture/conversations",
        data=json.dumps(event, separators=(",", ":")).encode("utf-8"),
        headers={"Authorization": "Bearer " + token, "Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=3) as response:
        return response.status in (200, 201)


def main():
    source = sys.argv[1] if len(sys.argv) > 1 else ""
    if source not in ("codex", "claude-code"):
        return
    try:
        hook = json.load(sys.stdin)
    except (ValueError, UnicodeDecodeError):
        return
    event = event_from_hook(source, hook)
    if event is None:
        return
    url = os.environ.get("OWNSTATE_CAPTURE_URL", "")
    token_file = os.environ.get("OWNSTATE_CAPTURE_TOKEN_FILE", "")
    if not url.startswith("https://") or not token_file:
        return
    try:
        path = pathlib.Path(token_file).expanduser()
        if path.stat().st_mode & 0o077:
            return
        token = path.read_text(encoding="utf-8").strip()
        if token:
            send(url, token, event)
    except (OSError, urllib.error.URLError, ValueError):
        pass


if __name__ == "__main__":
    main()
    # Codex Stop requires JSON; Claude Code accepts it. Never inject text.
    print("{}")
