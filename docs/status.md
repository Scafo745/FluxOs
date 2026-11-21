# Project status

Checkpoint: November 21, 2026, after the backend milestone. Requirements: [prd.md](prd.md). Design: [architecture.md](architecture.md). Endpoints: [api.md](api.md).

## Where we are

The backend is built and verified in development. The frontend (boot screen and desktop shell) is next.

| PRD phase | State |
|---|---|
| 0: Foundation (profile schema, tool registry, permission tiers) | Done |
| 1: Real Linux MVP (agent plus applier, snapshot and rollback, themes and design tokens, gaming profile) | Done on the sandbox target. The NixOS target is implemented and its output evaluates, but it has not run on a real NixOS host yet. |
| 2: Safety and profiles (sandbox layer, audit log, profile sharing, education profile, hardware checks) | Mostly done: audit log, signed profile sharing, education profile, hardware checks. MicroVM sandboxing is not built; FluxOS never runs model-generated code, so the sandbox stage is the sandbox target plus `nixos-rebuild build`. |
| 3: Images and enterprise (signed image builder, fleet management, telecom profile, admin approval flows) | Partly: telecom profile, fleet mode with admin approvals, change-freeze windows. No image builder or fleet management yet. |
| 4: Regulated verticals | Finance and healthcare ship as preview profiles that require compliance review. |
| 5: Platform | An MCP endpoint exists. Marketplace, WASM plugins and the developer API are not started. |

## What was built

- **fluxd** (Rust): the daemon and trust boundary.
  - Typed system spec, a default-deny tool registry (31 allowed tools, 6 blocked), and a policy engine that takes the risk tier from the actual state diff.
  - Approvals bound to a plan hash, with permissions checked again at apply time and plans re-validated against the live state.
  - Generations with snapshot, health checks and automatic rollback, plus crash recovery at boot.
  - Sandbox and NixOS apply targets.
  - Hash-chained audit log, with degraded mode when the chain is broken.
  - SSE events and a boot report for the UI.
  - Signed profile bundles and an MCP endpoint.
- **flux-agent** (TypeScript): the AI orchestrator.
  - A local rule engine handles common requests without an LLM call.
  - Anthropic, OpenAI-compatible and Ollama providers with failover and a circuit breaker.
  - BM25 retrieval over industry knowledge, clarifying questions, and a validate-and-repair loop.
  - Pipeline steps stream out as they happen.
- **Design system**: W3C DTCG 2025.10 tokens, contrast-safe palette generation, and exporters for NixOS, CSS, GTK, KDE, GNOME dconf and Hyprland. Wallpapers are original generated SVG.
- **Profiles**: default, gaming, education, telecom, finance (preview), healthcare (preview).
- **Bootable**: a NixOS module for both services (the agent is sandboxed with `DynamicUser`), and a flake with packages and a demo VM (`nix run .#vm`).
- **Tooling**: `scripts/dev.sh`, the `scripts/flux` CLI, and the `scripts/verify/` checks.

## Decisions and deviations

| Topic | Decision | Why |
|---|---|---|
| Name | FluxOS (repo name) | MorphOS is taken (PRD open question 1). Trademark search still needed. |
| Orchestrator language | TypeScript | The PRD allows Python or TypeScript; TypeScript shares types with the React frontend. |
| Base | NixOS 26.05, pinned in `flake.lock` | PRD open question 4: NixOS only for now. Fedora Atomic/bootc would be a new apply target. |
| Default desktop | KDE Plasma; GNOME for education and healthcare; Hyprland for telecom | PRD open question 5. COSMIC is supported, but its theme export is not written yet. |
| Hardened kernel | LTS kernel plus KSPP-style boot parameters and sysctls | nixpkgs 26.05 removed `linux_hardened`. |
| sched_ext | lavd, bpfland and rusty marked stable; flash, cosmos and p2dq marked experimental, with a warning | PRD open question 8. All six exist in the nixpkgs 26.05 `scx` package. Maturity on target hardware still needs testing. |
| AI mix | Local rules first, cloud LLM for novel requests, Ollama optional, `FLUX_AI_ROUTING=local` for on-device only | PRD open question 6 and the privacy and performance requirements. The cost budget is still open. |
| Default models | `claude-sonnet-5-5`, `gpt-6.2-sol`, `qwen3:8b`, all configurable | Current models as of November 2026. |
| First enterprise vertical | Both education and telecom are built | PRD open question 3 is still yours to decide. |
| Retrieval | BM25, no vector database | No external service needed. `Doc` in `agent/src/retrieval.ts` is where a vector DB plugs in. |
| Execution model | The model never produces scripts or code; it only picks typed operations | Removes the need to sandbox generated code (PRD 7.7) for now. |

## Verification

Run these to reproduce the checks. All passed at this checkpoint.

| Script | Checks | Covers |
|---|---|---|
| `scripts/verify/daemon.sh` | 72 | tiers and approvals, plan-hash binding, blocked tools and blocked diffs, validation errors, staged kernel change and simulated reboot, automatic rollback on failed health and failed activation, undo, MCP, signed profiles and tamper rejection, audit chain, SSE, crash recovery, audit tamper detection, fleet mode, freeze windows, operator-disabled tools |
| `scripts/verify/agent.sh` | 33 | local engine on about 20 request types, clarifying questions, undo; LLM path against a scripted mock: failover from a failing provider, repair loop, clarification, a prompt-injection attempt held for approval, circuit breaker |
| `scripts/verify/nix-eval.sh` | 25 | every profile and four edge cases evaluate as complete NixOS 26.05 systems with no warnings; the flake's packages and `fluxos-vm` evaluate |

```bash
cargo build -p fluxd
(cd agent && npm ci && npm run build)
scripts/verify/daemon.sh
scripts/verify/agent.sh
scripts/verify/nix-eval.sh   # needs nix; NIX="nix --store ..." and NIXPKGS=/path are optional overrides
```

Nix evaluation found two real renderer bugs, both fixed: NixOS 26.05 rejects untyped integers in dconf settings, and kiosk mode needs an XDG portal for Flatpak.

**Not verified yet**

- Booting the VM image and applying a generation inside it.
- `nixos-rebuild build/switch/boot` driven by fluxd on a real NixOS host.
- Nix builds of `fluxd` and `flux-agent`. Only evaluation ran, because the development environment had no build sandbox.
- Real Anthropic, OpenAI and Ollama endpoints (no keys were available).
- The Plasma layout script, GNOME extensions and Hyprland snippet on a live desktop.

## Frontend integration points

The backend already exposes what the boot screen and desktop need (details in [api.md](api.md)):

| UI feature | Backend |
|---|---|
| Boot screen | `GET /v1/boot` (stages with status, detail, timing) and SSE `boot.stage` / `boot.ready`. The replay buffer lets a UI that connects late still render the full boot. A simulated reboot replays the stages. |
| Live theming | `GET /v1/theme.css`, `/v1/tokens.json`, `/v1/wallpaper.svg`. Refetch on the `generation.switched` event so the UI changes with the OS. |
| Talking to the AI | `POST /v1/intents/stream` on flux-agent: `step` events (route, provider, validate, repair, propose, verify) then a `result`. Good for a live "AI thinking" view. |
| Approvals | Plan `change_summary` and `evaluation.changes` (riskiest first), `GET /v1/plans/{id}/preview?path=...` to show the new theme or wallpaper before approving, then `POST /v1/plans/{id}/approve` with the `plan_hash`. |
| History and undo | `GET /v1/generations`, `POST /v1/rollback`. |
| Activity and trust | `GET /v1/audit`, `/v1/audit/verify`, and live `apply.*` events. |

Known gap: the UI currently needs the admin token from `<state>/secrets/admin.token`. A pairing flow, such as a one-time code shown at boot, should replace that before any real deployment.

## Next steps

1. Frontend: boot screen and desktop shell (React, TypeScript, Vite per the PRD) on the APIs above.
2. UI pairing flow instead of reading token files.
3. CI: clippy, build, `tsc`, `scripts/verify/daemon.sh` and `agent.sh`, and `nix-eval.sh` on a runner with Nix.
4. Boot the VM and run a full `nixos-rebuild` cycle; build the Nix packages.
5. Smoke tests against real AI providers once keys are available.
6. A Plymouth theme generated from the design tokens, so real hardware boots with the same look as the web boot screen.
7. Later phases: COSMIC theme export, signed OCI/bootc images, central policy and fleet management, SSO, nftables/eBPF policy, WASM plugins.

## Open questions

1. Where is the existing web MVP (React, TypeScript, Vite, Tailwind, Zustand) from PRD section 15.4? Should the boot screen and desktop reuse it, or start fresh?
2. Is FluxOS the final name? The trademark search is still open.
3. Which enterprise vertical launches first: education or telecom?
4. What is the per-user AI inference budget? It decides how often requests may skip the local engine.
5. Team size and timeline, so the roadmap can get dates.
6. Who advises on compliance before the finance and healthcare profiles leave preview?
