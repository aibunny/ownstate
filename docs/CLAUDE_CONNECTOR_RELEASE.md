# Ownstate in Claude

## Intended user experience

Publish Ownstate as a single remote MCP connector in the Claude directory.
Each person selects Ownstate, signs in with their Ownstate account once, and
chooses a Personal or organization space. Claude stores the OAuth connection
for that Claude account; Ownstate refreshes its scoped access token until the
connection is revoked or its refresh token expires after 365 days of inactivity.
Claude may still require the person to enable the connector in individual chats.
No user copies a bearer token or shares a tenant-wide credential.

## Connector configuration

- Display name: Ownstate
- Remote MCP URL: `https://api-production-8889.up.railway.app/mcp`
- Authentication: OAuth 2.0, authorization code with PKCE
- OAuth client for a custom connector: **Register automatically**
- Fixed request headers: none
- Protected-resource metadata: `https://api-production-8889.up.railway.app/.well-known/oauth-protected-resource/mcp`
- Authorization-server metadata: `https://api-production-8889.up.railway.app/.well-known/oauth-authorization-server`

For an individual Claude account, add a custom connector under Customize →
Connectors and choose **Sign in now**. For a Team or Enterprise Claude account,
an organization owner adds the connector; each member then connects with their
own Ownstate account. This pilot is separate from a public directory listing.

## Verification before directory submission

1. Complete the live Claude OAuth flow and confirm an Ownstate OAuth grant was
   created for the intended account and workspace. Check the server received
   client registration, authorization, token exchange, and an authenticated MCP
   request. Test reuse in a new Claude session and token refresh after expiry.
2. Confirm the MCP tool annotations are deployed and Claude sees the intended
   tools. Test one context read and one explicitly requested evidence write.
3. Publish an accurate privacy policy covering account data, stored evidence,
   retention, and support requests. Identify a verified support contact.
4. Create a separate reviewer account with sample data and no production user
   content. Provide at least three working prompts using supported tools.
5. Verify publisher control of the endpoint/domain and submit the single MCP
   connector through Claude's directory portal. Complete Anthropic's review and
   publish only after approval.

Suggested review prompts, after seeding the reviewer account:

- “What project knowledge does Ownstate have about the sample project?”
- “Search Ownstate for the sample project's authentication decision and cite its evidence.”
- “Record this decision in the sample project: use OAuth for each user's connection.”

The connector exposes context and explicit records. Claude's hosted connector
does not provide automatic access to all chats or Claude memory. Local capture
adapters and authorized imports have separate, partial coverage; do not describe
the directory connector as a full transcript recorder.

References: [Claude custom connectors](https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp),
[Claude directory submissions](https://claude.com/resources/articles/build-plugins-for-claude),
[Anthropic Software Directory Policy](https://support.claude.com/en/articles/13145358-anthropic-software-directory-policy).
