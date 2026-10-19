# Ownstate privacy policy

Effective date: 13 October 2026

Ownstate is a pilot service for storing and retrieving context that people
choose to provide to AI tools. Questions and data requests can be sent to
[ownstate@theaibunny.com](mailto:ownstate@theaibunny.com).

## Information we handle

- **Account and access data:** email address, password hash, email verification
  status, login sessions, organization memberships, OAuth client registrations,
  grants, and connection records. Passwords and bearer tokens are not stored in
  plaintext by Ownstate.
- **Content you provide:** project details, interaction evidence, chat events,
  proposed knowledge, and associated metadata when you or an authorized client
  explicitly sends them to Ownstate. Organization members may access content
  within their authorized organization space.
- **Service records:** timestamps, request paths, status codes, and operational
  logs. Hosting logs may also contain an IP address and browser or client
  identifier.

Ownstate uses this information to authenticate users, separate personal and
organization spaces, provide requested context to connected tools, record
submitted evidence, maintain provenance, and diagnose service failures.

## Claude and other AI clients

When you connect an AI client, you choose the Ownstate space it can access.
Ownstate sends tool responses from that space to the client when its tools are
used. A client can also submit information through Ownstate's write tools.
You can revoke an account connection from the Ownstate account page.

The hosted Claude MCP connector does **not** give Ownstate automatic access to
all Claude chats, Claude memory, or uploaded files. Ownstate receives the
content sent through its tools. Local capture adapters, if separately enabled,
can send additional supported events; their coverage is partial and depends on
the host application.

## Service providers

Ownstate uses Railway to host the API, worker, and database, and Resend to
deliver account verification and invitation emails. Resend receives the email
address and message needed for delivery. Connected AI clients receive the
Ownstate tool responses requested through those clients. We do not provide
one user's Personal space to another user's connection.

## Retention and requests

Stored project and conversation evidence currently has **no automatic expiry**.
It remains until an authorized retention or deletion process is completed.
Ownstate does not yet offer self-service account or evidence deletion, and
revoking a connector stops future access without erasing previously stored
records. OAuth access tokens expire after one hour; refresh credentials rotate
and expire after 365 days of inactivity unless revoked earlier.

To request a copy, correction, or deletion of your data, email
[ownstate@theaibunny.com](mailto:ownstate@theaibunny.com) from your account
address. We will verify account ownership, assess the request, and explain what
can be changed or removed, including any limits from append-only evidence and
backups. Do not send passwords or tokens by email.

## Security and changes

The public service uses HTTPS. Ownstate checks account and space permissions
for authenticated requests and stores password and connection credential
digests rather than plaintext values. No system can guarantee perfect security.
We will update this page and its effective date if the handling described here
changes materially.
