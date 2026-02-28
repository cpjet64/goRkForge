# special.md

**goRkForge**  
**Groks Self-Building, Self-Improving, Platform-Native Coding Agent**  
Build the machine that builds the machine  then let the machine gork itself harder.  Elon (amplified)

**Version:** 2.3 (Feb 28 2026)  .env + dotenvy support added  
**Canonical source of truth.**

---

## Configuration

- Root `.env` (gitignored) with `xai_api_key=sk-...` (or `XAI_API_KEY=sk-...` fallback)
- `.gorkforge/policy.toml` for runtime policy
- Never commit secrets.

---

## Phase 0 Requirements (updated)

- Load `.env` automatically using `dotenvy`
- `gorkforge-core` exports `Config` struct with `xai_api_key`
- CLI prints  API key loaded on every run
- Everything else identical to v2.2

Once skeleton is built, I connect via xAI API and finish the full agent.
