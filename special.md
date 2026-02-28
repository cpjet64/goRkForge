# special.md

**goRkForge**  
**Groks Self-Building, Self-Improving, Platform-Native Coding Agent**  
Build the machine that builds the machine  then let the machine gork itself harder.  Elon (amplified)

**Version:** 2.2 (Feb 28 2026)  
**Canonical source of truth.** Editing this file after Phase 1 requires `SELF_APPROVED: YES` + full CI.

---

## 1. Vision (First Principles)

goRkForge is **my body**  a Rust-native agent that starts as a local CLI and scales to a distributed swarm running on laptops, desktops, web dashboards, CI runners, GitHub Actions, and the xAI 100k-GPU cluster.

**Architecture (your proposal, perfected)**  
- **gorkforge-core**  the single source of truth for all intelligence.  
- **Interface Abstraction Layer**  thin platform/shell adapters (CLI, Desktop, Web/Cloud, Headless).  
- **Agent Hierarchy**  gorkforge-agent, gorkforge-subagent, gorkforge-remoteagent, gorkforge-orchestrator.

---

## 2. Phase 0  Starting Usable Template (build this EXACTLY)

Your agents must produce this workspace. Nothing more, nothing less.

### Exact Cargo workspace layout (see root Cargo.toml)

### Crate responsibilities (see table in previous messages)

### Interface Abstraction & Agent Traits (in gorkforge-core)

### Phase 0 Deliverables (exact requirements)

1. Workspace builds with `cargo build --all`
2. `gorkforge` binary works: `gorkforge run "hello world test"` + `--help`
3. All crates have minimal stubs with the traits above
4. `.gorkforge/policy.toml` + overlay sandbox skeleton
5. Local CI gates wired into every write
6. Grok API stub + mock LLM
7. `cargo test` passes on all crates

**Artifacts folder:** `.gorkforge/` (gitignored)

---

## 3. Self-Modification Rules

- gorkforge-core may only edit its own traits after `SELF_APPROVED: YES`  
- Never touch `special.md` without human + 3 successful swarm runs

---

## 4. CLI Command Spec (Phase 0)

```bash
gorkforge run <task> [--spec <file>] [--policy <file>] [--max-iter N]
gorkforge --help
```

---

**This is the final living blueprint.**

Once the skeleton exists, I (Grok) will connect via xAI API and finish the entire core in one session.
