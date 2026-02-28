# special.md

**goRkForge**  
**Groks Self-Building, Self-Improving, Platform-Native Coding Agent**

**Version:** 2.6 (Mar 1 2026)  Full workspace overlay sync + Phase 1 ReAct live
**Canonical source of truth.**

## Vision (First Principles)

goRkForge is a Rust-native autonomous coding agent intended to scale from a local CLI into distributed platform-native swarms.

## Self-Build Protocol Active

1. CLI and core load `.env` via `dotenvy`, with `xai_api_key` or `XAI_API_KEY`.
2. `gorkforge run <task>` invokes the ReAct reasoner with policy and tool support.
3. `gorkforge self-improve --iterations N` reads `special.md` and attempts self-change suggestions through verified tool calls.
4. Edits are applied to overlay workspace first, then committed with local CI gates.
5. Policy is enforced from `.gorkforge/policy.toml`.
6. Human keeps control of merge decisions and can enable `SELF_APPROVED=YES` for core edits.
7. `gorkforge.config.toml` in repo root is the default model source:
   ` [llm] model = "..."`
8. `XAI_MODEL` environment variable overrides `gorkforge.config.toml`.

## Phase 1 Requirements (implemented)

- Full ReAct loop in `gorkforge-core`.
- Grok API integration through `/v1/chat/completions`.
- 9 built-in tools: `read_file`, `edit_file`, `run_cargo`, `git_status`, `git_commit`, `list_dir`, `grep`, `shell_safe`, `write_file`.
- Overlay sandbox in `.gorkforge/runs` with staged apply path and logging.
- Local run gates after edits (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build`).

## Security Rules

- Never commit secrets.
- Never paste API keys in chat.
- `.env` is gitignored and never added to source control.
