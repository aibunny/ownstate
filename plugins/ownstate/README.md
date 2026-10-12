# Ownstate plugin draft

This portable plugin package points ChatGPT and Codex at the public Ownstate
MCP endpoint. Each person authorizes a personal or organization space through
Ownstate's OAuth flow. Its dynamic client registration and rotating refresh
tokens allow the host to keep a connection across sessions until the person
revokes it or stops using it beyond the refresh lifetime.

The package is a draft for connection testing. Public submission still needs
the sending domain and signup enabled, a user connection tested in ChatGPT and
Codex, a verified publisher domain, privacy and retention documentation, and
the platform's review. The public plugin excludes local capture hooks because
plugin installation does not grant access to hosted chat transcripts. See
[`integrations/README.md`](../../integrations/README.md) for pilot local
capture adapters.
