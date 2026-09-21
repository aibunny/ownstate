# Ownstate website — build prompt

Build a premium, minimal public website for **Ownstate**.

Ownstate is open infrastructure for sovereign AI knowledge and institutional cognition. It is the durable, permission-aware memory and context layer beneath people, AI models, coding agents, agent fleets, vendors, and applications. It is not a chatbot, model provider, vector database, or agent framework.

## The job of the website

Make a serious buyer understand, within the first screen, that their knowledge belongs to them—not to the current model, agent, or SaaS tool—and that Ownstate lets them retain and govern it as their AI stack evolves.

The page must feel more credible, more restrained, and more institutionally ready than typical “AI memory” competitors. Do not mimic competitor copy, visual layouts, claims, or logos.

## Core positioning

**Primary message:** Your knowledge should outlive every model and every tool.

**Supporting message:** Connect the model you use today, bring in specialist agents tomorrow, or operate an agent fleet later. Ownstate remains the independent, governed memory layer beneath all of them.

**MCP message:** Ownstate MCP is the universal context and knowledge interface. A model or agent connects to it, bootstraps scoped context, retrieves evidence-grounded knowledge, and preserves valuable work as attributable evidence. Authorization and classification apply before context is delivered.

**Portability message:** Switching models changes the intelligence at the edge; it must not force an institution to lose its decisions, provenance, history, or accumulated knowledge.

## Product audiences and modes

Design the site to speak to two equally important audiences.

### Personal

For an individual who uses multiple AI tools, coding agents, and workspaces. Ownstate preserves useful knowledge across those tools so their context, preferences, decisions, artifacts, and lessons do not disappear when they change model or agent.

Use simple, human language: “Your work compounds, even when your tools change.”

### Institutional

For technology, operations, security, knowledge, and AI leaders. Ownstate represents institutional state: decisions, architecture, requirements, risks, relationships, commitments, artifacts, and the evidence that supports them.

Use specific, defensible language: evidence remains attributable; canonical knowledge is versioned rather than overwritten; provenance is inspectable; authorization happens before retrieval and model egress.

### Self-hosted and sovereign

Make ownership tangible. Ownstate is designed to run locally, in an organization’s own cloud, in a private institutional environment, or as a hosted service. The website should clearly state that hosted convenience must never become a hidden dependency.

Show three deployment choices without implying that one is superior:

1. Personal local environment
2. Your cloud / private deployment
3. Hosted convenience

Use the phrase: “Run where your institution needs its knowledge to live.”

## Information architecture

Create a one-page landing page with these sections:

1. **Hero** — “What your institution knows should outlive every tool.” Use a calm, monumental visual of durable memory rather than a dashboard or an AI character. Include a direct link to “See how it works.”
2. **The choice layer** — an image-led continuity field: preferred model, specialist agents, and an agent fleet converge at **Ownstate MCP**, which provides governed access to the institution’s evidence, knowledge, and provenance. Use an art-directed visual and restrained labels, never generic boxes, arrows, or a stock architecture diagram. This must be the site’s signature interaction or visual.
3. **How it works** — a four-step flow:
   - Connect Ownstate MCP
   - Bootstrap scoped context
   - Retrieve with authorization and provenance
   - Preserve evidence-backed knowledge
4. **Who it is for** — a concise personal / institutional split. Show different needs while making clear both use the same durable core.
5. **Where it runs** — local, private cloud, and hosted deployment choices. Emphasize sovereignty and portability.
6. **Trust architecture** — evidence, candidate knowledge, canonical knowledge, version history, provenance, and policy controls. Explain the distinction without introducing database jargon.
7. **Closing CTA** — “Keep the intelligence. Change everything else.” Do not invent an email address or external destination; use a placeholder anchor until a real contact route is supplied.

## Visual direction

The visual identity should feel like a contemporary research institute, a private archive, and well-engineered infrastructure—not like a startup marketing template.

- Palette: obsidian black `#090B0C`, graphite `#101516`, smoke gray, pale cyan `#B9ECE3`, and muted amber `#D9AD67` used only for evidence/status signals.
- Typography: elegant editorial serif for key claims; neutral sans-serif for reading; compact monospace for system labels and metadata.
- Signature visual: a smoked-glass archival monolith containing subtle cyan paths and amber provenance points. On desktop it belongs at the right side of the hero, leaving calm negative space for the headline on the left.
- Layout: strict grid, ample breathing room, hairline rules, no decorative noise.
- Motion: only slow ambient depth, a restrained connection animation through the MCP map, and measured content reveals. Respect `prefers-reduced-motion`.

Avoid cyberpunk, saturated neon, purple gradients, robots, AI faces, stock-photo teams, fake dashboards, generic rounded SaaS cards, excessive glassmorphism, fake customer logos, fake metrics, and unsupported compliance/security badges.

## Voice and content rules

- Speak plainly and specifically; never write empty “AI transformation” language.
- Use “evidence,” “knowledge,” “provenance,” “authorization,” “context,” “portable,” and “sovereign” with care and explanation.
- Never claim a brand-specific model integration unless it is verifiably implemented.
- Never claim lossless automatic capture, fully autonomous agents, production multi-provider integrations, or complete fine-grained model-egress enforcement unless current repository documentation proves it.
- Clearly distinguish implemented, experimental/local, and planned capabilities if a feature-status section is needed.
- Do not invent a contact email, pricing, customer roster, security certification, usage figures, or product results.

## Technical requirements

- Use semantic HTML, accessible keyboard focus, strong heading hierarchy, responsive layout, and `prefers-reduced-motion` support.
- Keep dependencies minimal; do not introduce a frontend framework unless the project has a clear need.
- Keep assets in `assets/` and document their origin.
- Consult the root repository `README.md` before changing product claims.
- Preserve the existing original hero asset: `assets/ownstate-institutional-memory-hero.png`.
- Use `assets/ownstate-continuity-field.png` for the image-led MCP continuity field. It is an original project asset, not a stock image.
