# Local chat capture pilots

These adapters use the user-owned connection token from the Ownstate account page. They capture observed local client events; an MCP connection alone does not provide host transcripts. The adapters are separate from the remote MCP connector and must be installed on each machine where local capture is desired.

1. Create a connection for the intended personal or organization space on the Ownstate account page. Save its token to a user-owned file with mode `0600`. Set `OWNSTATE_CAPTURE_URL` to the public HTTPS API origin and `OWNSTATE_CAPTURE_TOKEN_FILE` to that file's absolute path in the client process environment. Do not put the token in project config or source control.
2. For Codex, adapt [`codex-hooks.example.json`](hooks/codex-hooks.example.json) with the absolute script path, then merge those entries into `~/.codex/hooks.json`. For Claude Code, adapt [`claude-settings.example.json`](hooks/claude-settings.example.json) and merge the hooks into `~/.claude/settings.json`. Existing hooks should be preserved.
3. For OpenCode, copy [`ownstate-capture.js`](opencode/ownstate-capture.js) to `~/.config/opencode/plugins/ownstate-capture.js`. OpenCode loads global plugins at startup.
4. Restart the client. The server lists captured threads at `GET /capture/conversations` and events at `GET /capture/conversations/{id}/events`, using the same connection token. Revoke a connection from the account page to stop both MCP and capture access immediately.

Codex and Claude Code hooks capture submitted user prompts, final assistant messages, and tool names. They do not parse transcript files or send tool arguments/results, attachments, intermediate reasoning, or all streamed messages. OpenCode's plugin reads persisted user and assistant text parts when a session becomes idle. Repeated OpenCode deliveries use stable event IDs. Failed hook requests currently do not queue for later delivery. Capture is therefore partial and must be tested on each installed client version before making stronger claims.

Sources: [Codex hooks](https://learn.chatgpt.com/docs/hooks), [Claude Code hooks](https://code.claude.com/docs/en/hooks), [OpenCode plugins](https://docs.opencode.ai/docs/plugins/), and [OpenCode SDK](https://docs.opencode.ai/docs/sdk/).
