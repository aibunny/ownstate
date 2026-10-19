// Install as ~/.config/opencode/plugins/ownstate-capture.js after configuring
// OWNSTATE_CAPTURE_URL and OWNSTATE_CAPTURE_TOKEN_FILE in the OpenCode process.
// OpenCode's session.idle event lets us read persisted final message parts.
import { readFile, stat } from "node:fs/promises"

export const OwnstateCapture = async ({ client }) => {
  const endpoint = process.env.OWNSTATE_CAPTURE_URL
  const tokenFile = process.env.OWNSTATE_CAPTURE_TOKEN_FILE
  if (!endpoint?.startsWith("https://") || !tokenFile) return {}

  const capture = async (sessionID) => {
    try {
      const metadata = await stat(tokenFile)
      if (metadata.mode & 0o077) return
      const token = (await readFile(tokenFile, "utf8")).trim()
      if (!token) return
      const response = await client.session.messages({ path: { id: sessionID } })
      const messages = response.data ?? []
      for (const message of messages) {
        const role = message.info?.role
        if (role !== "user" && role !== "assistant") continue
        for (const part of message.parts ?? []) {
          if (part.type !== "text" || !part.text || !part.id) continue
          const body = {
            source: "opencode",
            source_conversation_id: sessionID,
            source_event_id: `${message.info.id}:${part.id}`,
            capture_mode: "PARTIAL_ADAPTER",
            event_type: role === "user" ? "USER_MESSAGE" : "ASSISTANT_MESSAGE",
            actor_type: role === "user" ? "USER" : "ASSISTANT",
            content: part.text,
          }
          const result = await fetch(`${endpoint.replace(/\/$/, "")}/capture/conversations`, {
            method: "POST",
            headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
            body: JSON.stringify(body),
            signal: AbortSignal.timeout(5000),
          })
          if (!result.ok) return // retry the full session on its next idle event
        }
      }
    } catch {
      // Capture never blocks the host session. The next idle event retries.
    }
  }
  return {
    event: async ({ event }) => {
      if (event.type === "session.idle" && event.properties?.sessionID) {
        await capture(event.properties.sessionID)
      }
    },
  }
}
