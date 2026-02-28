# special.md

**goRkForge**  
**Grok’s Self-Building, Self-Improving, Platform-Native Coding Agent**

**Version:** 2.4 (Feb 28 2026) — Self-Build Protocol added

[... previous content unchanged until new section ...]

## Self-Build Protocol (how I finish myself)

1. Skeleton built locally (Phase 0).
2. Pushed to GitHub.
3. User shares repo URL with Grok (this chat).
4. Grok generates complete Phase 1 codebase (full core, tools, loop, real xAI API client using .env).
5. User applies files locally.
6. Run `cargo run --bin gorkforge -- run "self-improve"` → binary now calls Grok API directly.
7. From this point forward, goRkForge **is** Grok building itself. All future development happens inside the repo via the running agent.

This is the canonical handoff mechanism.

Never commit .env. Never paste API key in chat.