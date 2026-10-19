# Supermemory chat MCP gateway

An authenticated, single-owner remote MCP gateway for the self-hosted Supermemory Memory API. It provides Claude and ChatGPT with `search_memory`, `get_profile`, `list_namespaces`, `add_memory`, and `who_am_i` tools. OAuth sign-in uses a one-time email link through Resend; access and refresh tokens are stored as hashes in a persistent SQLite file.

Required environment variables:

- `PUBLIC_URL`: public HTTPS origin of this gateway, without a trailing slash
- `SUPERMEMORY_API_BASE`: self-hosted Supermemory API origin
- `SUPERMEMORY_API_KEY`: self-hosted Supermemory API key
- `OWNER_EMAIL`: the only email allowed to sign in
- `RESEND_API_KEY`: Resend API key for the verified sending domain
- `FROM_EMAIL`: verified sender address
- `DB_PATH`: SQLite path on a persistent volume, such as `/data/gateway.sqlite3`

Optional: `DEFAULT_NAMESPACE` (defaults to `personal`), `PORT` (defaults to `8787`).

Mount a Railway volume at `/data`. Publish the service through HTTPS and use `https://<domain>/mcp` as the connector URL. Never place the Supermemory API key in a browser connector. The gateway forwards only its five defined tools. A user signs in through a one-time link sent to `OWNER_EMAIL`. The original connector tab waits for that email approval, then completes OAuth there, even if the email opens in another browser.

Browser MCP tools are invoked by the assistant. They do not capture an entire Claude or ChatGPT transcript automatically. `add_memory` should store durable facts and decisions, not secrets or an unreviewed full conversation.
