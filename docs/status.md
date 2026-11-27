# Project status

Checkpoint: November 21, 2026, after the frontend milestone. Requirements: [prd.md](prd.md). Design: [architecture.md](architecture.md). Endpoints: [api.md](api.md).

## Where we are

The backend and the web UI are built and verified in development. fluxd serves the UI, a browser pairs with a one-time code, and the desktop shell is driven entirely by the real fluxd and agent APIs. What remains unverified is everything that needs a real NixOS machine: Nix builds, booting the VM, and `nixos-rebuild` cycles.

| PRD phase | State |
|---|---|
| 0: Foundation (profile schema, tool registry, permission tiers) | Done |
| 1: Real Linux MVP (agent plus applier, snapshot and rollback, themes and design tokens, gaming profile) | Done on the sandbox target. The NixOS target is implemented and its output evaluates, but it has not run on a real NixOS host yet. |
| 2: Safety and profiles (sandbox layer, audit log, profile sharing, education profile, hardware checks) | Mostly done: audit log, signed profile sharing, education profile, hardware checks. MicroVM sandboxing is not built; FluxOS never runs model-generated code, so the sandbox stage is the sandbox target plus `nixos-rebuild build`. |
| 3: Images and enterprise (signed image builder, fleet management, telecom profile, admin approval flows) | Partly: telecom profile, fleet mode with admin approvals, change-freeze windows. No image builder or fleet management yet. |
| 4: Regulated verticals | Finance and healthcare ship as preview profiles that require compliance review. |
| 5: Platform | An MCP endpoint exists. Marketplace, WASM plugins and the developer API are not started. |
| UI: demo and admin console (PRD 15.4) | Built: boot screen, pairing, desktop shell, assistant and admin apps. |

## What was built

### Web UI (this checkpoint)

`ui/`: React 19, TypeScript, Vite 8, Tailwind 4, Zustand 5, lucide icons. JavaScript bundle: 375 KB, 115 KB gzipped.

- **Boot screen.** It loads the live theme and wallpaper, then replays fluxd's real boot stages with their details and measured durations. `Space` skips, `Esc` shows details. A simulated reboot streams its own stages (firmware, bootloader, kernel, scheduler, session, health) into the same view, and a fluxd restart is detected through `boot_id`.
- **Pairing.** A pairing screen takes the one-time code, and `/#pair=CODE` links pair automatically. Wrong codes show the attempts left. A screen that is already paired drops pairing links. A revoked screen returns to the pairing screen.
- **Desktop shell from the spec.**
  - Theme tokens become CSS custom properties, and theme changes animate in.
  - The dock follows the spec's position, size, auto-hide and floating settings.
  - The panel sits at the top or bottom, with the global menu and a taskbar when the dock is off.
  - Windows follow the spec too: tiling (master and stack, the spec's gaps), snapping and workspaces.
  - Ctrl+K opens the assistant, and the panel shows the AI status, pending approvals and the generation.
- **Assistant.** Example prompts. A live trace of the agent's pipeline steps. Clarifying questions with options. Results showing fluxd's change list, then approve, preview, reject, undo and restart.
- **Apps.**
  - Approvals: the change list, AI text labeled as context, pre-flight checks, operations and the plan hash.
  - Timeline: generations with restore, preview and the generated NixOS config.
  - System: overview, design tokens with contrast grades, generated config files, paired screens.
  - Profiles: review and apply, signed export and import, save the current setup.
  - Activity: the audit log with chain verification, and live events.
  - Also quick settings (dark or light, accent, wallpaper, style, motion, high contrast, large text, reduced motion) and a launcher with every installed app.
- **Preview.** A plan or generation preview swaps in the candidate spec, theme and wallpaper without applying anything, with an approve or restore button in the banner.
- **Restore safety.** Restoring a generation dry-runs `generation.restore` first. Tier 2 or higher opens a dialog with the exact changes.

### fluxd changes for the UI

- Pairing with one-time codes and revocable session tokens (`/v1/pair`, `/v1/logout`, `/v1/sessions`, `/v1/sessions/{id}/revoke`, `fluxd pair-code`).
- `--ui-dir` serves the built UI with a strict CSP. `/v1/ui-config` exists, and unknown `/v1/*` paths return a JSON 404.
- A Host-header guard against DNS rebinding (421 `host_not_allowed`, `--allowed-hosts`).
- `boot_id` and `event_seq` in health. Event cursors from an earlier daemon instance reset.
- Plan and apply events carry their origin (`source`, `session_id`), and rollback accepts a `session_id`.
- Titled stages for the simulated reboot.
- Theme names are restricted to letters, digits and simple punctuation, with escaping in the CSS and SVG renderers as a second layer. Before this, a tier 0 plan (applied without asking) could inject CSS into `theme.css` or a script into `wallpaper.svg`, which the UI loads from fluxd's origin.

### Packaging and tooling

- `nix/flux-ui.nix` builds the UI. The NixOS module serves it by default (`services.fluxos.ui.enable`, `ui.package`) and adds a "FluxOS" launcher entry. It passes the agent URL to fluxd, allows the UI origin in the agent's CORS, and gains `allowedHosts`.
- The demo VM opens the UI at login, already paired. This works because its auto-login user is an administrator anyway.
- `scripts/dev.sh` runs the UI from the Vite dev server (default), as fluxd-served static files (`FLUX_UI=dist`), or not at all (`FLUX_UI=none`), and prints a pairing link. `scripts/flux pair` prints the current code and link.

### From the backend milestone

- **fluxd** (Rust): the daemon and trust boundary.
  - Typed system spec, a default-deny tool registry (31 allowed tools, 6 blocked), and a policy engine that takes the risk tier from the actual state diff.
  - Approvals bound to a plan hash, with permissions checked again at apply time and plans re-validated against the live state.
  - Generations with snapshot, health checks and automatic rollback, plus crash recovery at boot.
  - Sandbox and NixOS apply targets.
  - Hash-chained audit log, with degraded mode when the chain is broken.
  - SSE events and a boot report.
  - Signed profile bundles and an MCP endpoint.
- **flux-agent** (TypeScript): the AI orchestrator. A local rule engine for common requests; Anthropic, OpenAI-compatible and Ollama providers with failover and a circuit breaker; BM25 retrieval over industry knowledge; clarifying questions; a validate-and-repair loop; streamed pipeline steps.
- **Design system**: W3C DTCG 2025.10 tokens, contrast-safe palette generation, and exporters for NixOS, CSS, GTK, KDE, GNOME dconf and Hyprland. Wallpapers are original generated SVG.
- **Profiles**: default, gaming, education, telecom, finance (preview), healthcare (preview).
- **Bootable**: a NixOS module for both services (the agent is sandboxed with `DynamicUser`), and a flake with packages and a demo VM (`nix run .#vm`).

## Decisions and deviations

| Topic | Decision | Why |
|---|---|---|
| Name | FluxOS (repo name) | MorphOS is taken (PRD open question 1). Trademark search still needed. |
| UI codebase | Built fresh with the PRD's stack (React, TypeScript, Vite, Tailwind, Zustand) | The existing MVP from PRD 15.4 is not in this repository. Open question 1. |
| How browsers authenticate | One-time pairing code from the fluxd log, exchanged for a revocable session token | A static admin token in a web page would be a master key. The agent never sees human tokens; the pairing response gives the UI the agent's intent token. |
| Where the UI is served | By fluxd on its own origin; Vite proxy in development | Same-origin API calls and a strict CSP. The agent stays a separate origin with CORS. |
| Event stream | `fetch` streaming with an `Authorization` header, starting at `event_seq` | Keeps tokens out of URLs and logs; no replay of old events on connect. |
| DNS rebinding | Host-header allowlist on every route | Listening on loopback alone does not stop a malicious page in a local browser. |
| Fonts and icons | Fontsource fonts bundled (OFL, Latin subset); app icons are original glyphs per category | No third-party requests and no third-party logos. |
| Staged generations | The new theme shows at once, with a banner saying a restart is needed for kernel and boot changes | Matches what fluxd materializes, and stays consistent across page reloads. |
| Demo VM pairing | The VM hands the pairing link to its auto-login user at boot | That user is already an administrator; real installs pair with `sudo fluxd pair-code`. |
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
| `scripts/verify/daemon.sh` | 104 | tiers and approvals, plan-hash binding, blocked tools and blocked diffs, validation errors, staged kernel change and simulated reboot, automatic rollback on failed health and failed activation, undo, MCP, signed profiles and tamper rejection, audit chain, SSE, crash recovery, audit tamper detection, fleet mode, freeze windows, operator-disabled tools; new: pairing (code never served, single use, lockout, fleet role), session listing and revocation, Host guard, sandboxed artifacts, theme-name injection, event cursors, UI serving with its CSP |
| `scripts/verify/agent.sh` | 33 | local engine on about 20 request types, clarifying questions, undo; LLM path against a scripted mock: failover from a failing provider, repair loop, clarification, a prompt-injection attempt held for approval, circuit breaker |
| `scripts/verify/nix-eval.sh` | 29 | every profile and four edge cases evaluate as complete NixOS 26.05 systems with no warnings; the flake's packages (including `flux-ui`) and `fluxos-vm` evaluate; the VM serves the UI package, allows its origin in the agent, and opens it at login |
| `cd ui && npm run build` | | `tsc` type check and production build |

```bash
cargo build -p fluxd
(cd agent && npm ci && npm run build)
(cd ui && npm ci && npm run build)
scripts/verify/daemon.sh
scripts/verify/agent.sh
scripts/verify/nix-eval.sh   # needs nix; NIX="nix --store ..." and NIXPKGS=/path are optional overrides
```

The UI was also driven end to end in headless Chromium at 1440×900, against fluxd serving `ui/dist` and the real agent. That script is not in the repository, because no UI test suite was requested. The run covered:

- boot and automatic pairing, then a neon theme and the glass style through the assistant
- the gaming plan: preview, approval from the banner, staging, and the simulated reboot
- every app, quick settings, and the telecom profile in the tiling layout
- the restore confirmation dialog and the launcher
- manual pairing on a second screen, with a wrong code first

It ended with no console errors. The only message was the expected 403 from the deliberately wrong code. The audit chain verified afterwards.

That run found and fixed these bugs:

- The arrow in change summaries rendered as an empty box, because the Latin font subset has no U+2192. It is now drawn as an icon.
- The assistant kept offering "Approve & apply" after the plan was approved elsewhere.
- Old result cards offered a restart after the staged generation was already running.
- Timeline summaries were not clamped.
- The theme-name injection described above.

Earlier, Nix evaluation found two renderer bugs, both fixed: NixOS 26.05 rejects untyped integers in dconf settings, and kiosk mode needs an XDG portal for Flatpak.

**Not verified yet**

- Booting the VM image, including the UI autostart and demo pairing, and applying a generation inside it.
- `nixos-rebuild build/switch/boot` driven by fluxd on a real NixOS host.
- Nix builds of `fluxd`, `flux-agent` and `flux-ui`. Only evaluation ran, because the development environment has no build sandbox. A plain `npm ci && npm run build` of exactly the files `flux-ui.nix` packages did succeed.
- The UI in Firefox, Safari or on touch screens, and with a screen reader. Only Chromium ran.
- Real Anthropic, OpenAI and Ollama endpoints (no keys were available).
- The Plasma layout script, GNOME extensions and Hyprland snippet on a live desktop.

## Next steps

1. CI: clippy, build, `tsc` for the agent and the UI, `scripts/verify/daemon.sh` and `agent.sh`, and `nix-eval.sh` on a runner with Nix.
2. Boot the VM and run a full `nixos-rebuild` cycle; build the Nix packages.
3. Decide whether the headless UI run becomes a committed end-to-end test (Playwright).
4. A Plymouth theme generated from the design tokens, so real hardware boots with the same look as the web boot screen.
5. Smoke tests against real AI providers once keys are available.
6. UI follow-ups: an accessibility pass with a screen reader, Firefox and touch layouts, translations.
7. Later phases: COSMIC theme export, signed OCI/bootc images, central policy and fleet management, SSO, nftables/eBPF policy, WASM plugins.

## Open questions

1. The existing web MVP from PRD section 15.4 is not in this repository, so the UI was built fresh. Is there anything in it (components, its heuristic engine) worth porting?
2. Is FluxOS the final name? The trademark search is still open.
3. Which enterprise vertical launches first: education or telecom?
4. What is the per-user AI inference budget? It decides how often requests may skip the local engine.
5. Team size and timeline, so the roadmap can get dates.
6. Who advises on compliance before the finance and healthcare profiles leave preview?
