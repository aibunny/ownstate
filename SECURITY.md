# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Ownstate, please report it privately.

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, please email: [security@ownstate.dev](mailto:security@ownstate.dev)

Include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

We will acknowledge receipt within 48 hours and provide a more detailed response within 7 days.

## Sensitive Areas

The following areas are especially security-sensitive:

- **Authorization and policy enforcement** — controls who can access what
- **Tenant isolation** — prevents data leakage across tenants
- **Model egress policy** — controls where data can be sent
- **Credential handling** — API tokens, database credentials, connector secrets
- **Input validation** — prevents injection and abuse
- **Context leakage** — ensures context is not shared across authorization boundaries
- **Scope handles** — opaque references must not leak internal state

## What to Report

Please report:

- Authorization bypasses
- Data leakage across tenants
- Credential exposure in logs or responses
- Injection vulnerabilities
- Path traversal or file system access
- Denial of service vectors
- Supply chain concerns

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Security Design Principles

Ownstate follows these security principles:

1. **Default deny** — all access requires explicit authorization
2. **Append-only evidence** — raw evidence cannot be modified or deleted
3. **Immutable canonical versions** — promoted knowledge cannot be rewritten
4. **Server-side authentication** — client claims never establish identity
5. **Model untrusted** — AI output is treated as untrusted input
6. **Least privilege** — grants are scoped to specific actions and resources
7. **No deletion through MCP** — the MCP interface cannot delete data

## Disclosure Policy

We follow coordinated disclosure:

1. Reporter reports privately
2. We acknowledge and investigate
3. We develop and test a fix
4. We release the fix
5. We publicly disclose the vulnerability

We request that reporters give us 90 days to address a vulnerability before public disclosure.
