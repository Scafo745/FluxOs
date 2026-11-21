# FluxOS architecture

This document explains how a plain-language request becomes a running system, where each safety guarantee from the PRD is enforced, and how the web UI fits in.

## 1. Components and trust

| Process | Privilege | Trusted for | Not trusted for |
|---|---|---|---|
| `fluxd` | root on a real machine | validation, approvals, applying, audit | nothing it receives |
| `flux-agent` | unprivileged, systemd `DynamicUser`, sandboxed | interpreting intent, drafting plans | any decision about risk |
| LLM providers | external or local | proposing operations | everything |
| web UI (a paired browser) | a revocable session token for one person | presenting plans to people, collecting their decisions | bypassing policy |
| CLI / MCP clients | a static human or agent token | scripting and automation | bypassing policy |

The agent authenticates to fluxd with the `agent` role. It can read state, propose plans and apply tier 0-1 plans. It cannot approve, cannot apply tier 2+, and never sees a human token: the UI sends approvals straight to fluxd. A compromised or prompt-injected agent can therefore, at worst, propose a plan that a person then sees and rejects.

Roles in fluxd (`crates/fluxd/src/auth.rs`):

- `admin`: device owner in personal mode, administrator in fleet mode. Approves tiers 0-3.
- `user`: exists in fleet mode. Approves tiers 0-1 only.
- `agent`: the orchestrator. Approves nothing.

Tokens are generated at first boot into `<state>/secrets/` with mode 0600 and compared in constant time. `intent.token` is what UI clients present to the agent; it carries no fluxd authority.

Browsers do not read token files. A person pairs a screen by entering a one-time code that fluxd prints to its log and keeps in `secrets/pairing.code`. No API returns the code, so a web page cannot pair itself. The screen receives a session token (`flxs_…`) bound to that person's role: the owner in personal mode, a local user in fleet mode, so administrators keep their own credentials. fluxd stores only the token's SHA-256, lists sessions to administrators, and revokes them on request. Codes work once; five wrong tries rotate the code and pause pairing for 30 seconds.

fluxd listens on loopback, which does not stop a malicious web page on the same machine from trying: through DNS rebinding the browser would talk to 127.0.0.1 under the attacker's hostname. A Host-header check in front of every route accepts only loopback names (and `--allowed-hosts`) and answers 421 otherwise.

## 2. The system spec

`SystemSpec` (`crates/flux-core/src/spec.rs`) is the declarative description of a machine, matching the PRD's profile contents:

- `base`: distro adapter (`nixos`), release, hostname, time zone, locale
- `boot`: Plymouth splash and theme, loader timeout
- `kernel`: variant from the tested catalog, allowlisted sysctls, CPU governor, I/O scheduler, sched_ext scheduler
- `drivers`: GPU driver, controller support
- `packages`: system packages (Nix) and user apps (Flathub), both by catalog id
- `services`: catalog ids
- `security`: firewall ports (the firewall itself is always on), AppArmor, USB storage, screen lock, audit
- `session`: standard, kiosk or exam; ephemeral home for shared machines
- `desktop`: environment, style preset, dock, panel, windows, workspaces, wallpaper
- `design`: theme name, design tokens, icon and cursor themes
- `accessibility`: high contrast, large text, text scale, screen reader, reduced motion

Every field is typed with `deny_unknown_fields`. Profiles (`profiles/*.json`) are JSON merge-patch overlays on the default spec plus optional style and palette seeds, materialized and validated at boot.

## 3. Tools and the policy engine

The registry (`registry.rs`) lists 31 allowed tools with a declared tier and 6 blocked ones. A plan is a list of `{tool, params, reason}`. `policy::evaluate` (`policy.rs`) is the trust boundary:

1. **Default-deny lookup.** Unknown tools are errors, with "did you mean" suggestions for the AI's repair loop. Blocked tools mark the plan tier 4. Operator-disabled tools (`policy.json`) are errors.
2. **Typed parameters.** Params are parsed into Rust structs that reject unknown fields.
3. **Simulation.** Each operation is applied to a copy of the current spec (`ops.rs`). Operations never run commands; they only edit the copy. Catalog lookups (packages, services, profiles) happen here.
4. **Whole-spec validation** (`validate_spec`): ranges, hex colors, open-licensed font catalog, icon and cursor catalogs, sysctl allowlist with value rules, kiosk URL strictness (https only, no shell, systemd or Nix metacharacters), WCAG contrast (4.5:1, or 7:1 in high contrast), Flatpak dependency, firewall and audit must stay on, hardware compatibility (no NVIDIA driver without an NVIDIA GPU; Secure Boot warnings).
5. **Diff and classification** (`diff.rs`): the current and candidate specs are diffed field by field. Each change gets a tier from its path and values (for example `security.firewall.enabled: true → false` is tier 4, `kernel.variant` is tier 3, `packages.flatpak` is tier 1) and a plain-language summary.
6. **Effective tier** = max(declared tool tiers, change tiers). A low-tier tool cannot cause a high-tier change without being classified as one.
7. **Decision** (`decide`): tier 0-1 auto-apply; tier 2-3 confirm (personal) or admin approval (fleet); tier 4 blocked.

The summary a person approves comes from step 5, not from the model. The model's own summary and rationale are stored separately and labeled untrusted.

## 4. Plan lifecycle in fluxd

`crates/fluxd/src/engine.rs`:

```
propose ──► proposed ──approve(plan_hash)──► approved ──apply──► applied | staged
   │            │                                           ├──► rolled_back (health failed)
   │            └──reject──► rejected                       └──► failed (activation failed)
   ├──► invalid / blocked / no_op (recorded and audited)
```

Apply, serialized by a lock:

1. Re-check permissions for the caller and the approval (role, tier, plan hash, 24 h expiry).
2. Refuse during an active change-freeze window for that tier.
3. Refuse tier 2+ if the daemon is degraded (for example a broken audit chain).
4. Re-evaluate the operations against the **live** state. If the plan no longer validates or now needs a higher tier, it must be proposed again.
5. Render generation N, run artifact checks (Nix syntax, DTCG tokens, contrast, sysctl lines, SVG).
6. Write generation N to disk next to N-1 (the snapshot), write an in-flight marker.
7. Activate through the target. On error: restore N-1.
8. Run target health checks. On any failure: restore N-1 automatically, mark N `rolled-back`.
9. Commit: switch `current`, mark N-1 `superseded`, clear the marker, audit.

Undo is a normal plan with `generation.restore`; its tier is computed from what it changes back, so undoing a security change still needs a person.

### Boot and crash recovery

`boot.rs` runs timed stages (state, policy, credentials, audit chain verification, registry, catalogs, profiles, hardware probe, target, crash recovery, generation, health, API) and emits each as a `boot.stage` event; `/v1/boot` returns the report. If an in-flight marker exists, the previous apply was interrupted: the last committed generation is restored and the attempt is marked failed. If the sandbox tree drifted from its generation, it is re-materialized. A failed audit-chain verification puts the daemon in degraded mode.

## 5. Targets

`crates/fluxd/src/targets/`:

- **sandbox** materializes each generation into `<state>/sandbox/root` with an atomic directory swap and keeps simulated boot entries for every generation. Health checks verify every file's SHA-256 against the generation manifest. This is the development target and the preview stage.
- **nixos** writes `generated.nix`, runs `nixos-rebuild build` (sandbox test: nothing activates), then `switch`, or `boot` for kernel and boot changes. Failures restore the previous file and configuration. Health checks use `systemctl is-system-running`, the display manager or kiosk unit, the default route, and the units of enabled services. NixOS keeps previous generations in the boot menu.

The `Target` trait is the adapter layer the PRD asks for; Fedora Atomic (bootc) or Btrfs-snapshot distributions would be new implementations.

## 6. Renderers and design tokens

`crates/flux-core/src/render/` turns a spec into a deterministic artifact tree; the same tree is previewed, stored per generation and materialized:

| Artifact | Consumer |
|---|---|
| `etc/nixos/fluxos/generated.nix` | NixOS (embeds every asset below via `environment.etc`) |
| `etc/fluxos/tokens.json` | W3C DTCG 2025.10 tokens (color objects, dimension and duration objects, cubicBezier, typography and shadow composites) |
| `etc/fluxos/theme.css` | web UI, Electron apps (`--flux-*` custom properties) |
| `etc/fluxos/gtk-4.0.css` | GTK 3/4 and libadwaita named colors |
| `etc/xdg/kdeglobals`, `usr/share/color-schemes/FluxOS.colors`, `etc/xdg/kwinrc` | Plasma and Qt |
| `etc/dconf/db/local.d/00-fluxos` | GNOME (rendered as typed `programs.dconf` settings on NixOS) |
| `etc/xdg/hypr/fluxos.conf` | Hyprland (bezier curve from the motion tokens) |
| `usr/share/plasma/layout/fluxos-layout.js` | Plasma panel and dock layout script |
| `etc/fluxos/wallpaper.svg` | original generated wallpaper |

Palettes come from `tokens::generate_palette`: one accent (plus optional secondary) in, a full palette out, with contrast enforced by adjusting lightness.

## 7. The AI orchestrator

`agent/src/pipeline.ts` implements the profile compiler:

1. **Intake**: sanitize the prompt; load session memory.
2. **Context**: tool registry, catalogs, profiles and hardware (cached), current system (per request).
3. **Retrieve**: BM25 over `agent/knowledge/*.md`, profiles, catalog entries and tool descriptions. The `Doc` interface is the seam for a vector database.
4. **Route**: the local rule engine (`heuristic.ts`) maps common requests and reports coverage, the share of meaningful words it understood. At or above `FLUX_FAST_PATH_COVERAGE` (0.6) it answers without an LLM call; otherwise the LLM chain runs. With no provider configured, the local result is used when there is one.
5. **Generate**: the system prompt carries the tool signatures, catalogs, current state and references. Retrieved text and history are fenced as untrusted. The reply is a JSON draft: `change`, `question`, `clarify` or `rollback`.
6. **Clarify**: questions only when the answer changes the outcome (shared lab vs personal device; a missing kiosk URL).
7. **Validate and repair**: dry-run validation in fluxd; errors (with suggestions) go back to the model up to `FLUX_MAX_REPAIRS` times. A local plan that fails validation is handed to the LLM with fluxd's feedback.
8. **Propose**: always through fluxd's real endpoint so even refused plans are audited; tier 0-1 auto-apply.

Providers (`agent/src/providers/`) share one interface. The router tries them in order, trips a per-provider circuit after three failures in two minutes, and records each attempt in the trace. Successful LLM plans are cached by prompt, answers and spec digest.

## 8. The web UI

`ui/` is a React 19 and TypeScript single-page app (Vite, Tailwind 4, Zustand). In production fluxd serves it on its own origin (`--ui-dir`), so the API is same-origin and the Content-Security-Policy can be strict: scripts only from fluxd, network access only to fluxd and the agent's origin, no frames, no plugins. The agent is a second origin and allows the UI through CORS. Fonts are self-hosted (Fontsource, OFL), so the UI makes no third-party requests.

**Boot.** Before pairing, the UI uses only public endpoints. It loads the live `theme.css` and wallpaper so the first screen already matches the system. Then it replays `/v1/boot`: the real stages, details and measured durations, paced for reading (`Space` skips, `Esc` shows details). Then it shows the pairing screen or the desktop. A simulated reboot on the sandbox target streams its own stages (firmware, bootloader, kernel, scheduler, session, health) into the same view.

**State.** `state/system.ts` holds the system view, plans, generations and events. One event stream keeps it current. The stream is `fetch` with the session token in the `Authorization` header, starting at `since=event_seq` from `/v1/health`. Plan events refresh the plan list, `generation.switched` reloads the system and theme, and a `boot.begin` with a new `boot_id` means fluxd restarted. Events carry `origin.session_id`, so the UI notifies a person only about changes started elsewhere (another screen, the CLI, MCP).

**The desktop follows the spec.** `theme.css` provides every design token as a CSS custom property, and the shell lays itself out from `spec.desktop`: dock position, size, auto-hide and floating; panel position and height, global menu, taskbar when there is no dock; tiling with the spec's gaps, snapping, and workspaces. A plan or generation preview swaps in the candidate spec, theme and wallpaper from fluxd's preview endpoints without applying anything.

**Decisions stay in fluxd.** The UI shows fluxd's change list (from the real diff), not the model's text, which is labeled as context only. Approving sends the `plan_hash` the person saw. Restoring a generation first dry-runs `generation.restore` through `/v1/plans/validate`; when going back is tier 2 or higher, a dialog shows the exact changes before anything happens. Buttons follow the live plan record, so a plan approved on one screen is shown as decided everywhere.

**Untrusted text.** Everything from fluxd and the agent is rendered as text, never as HTML. The live `theme.css` is applied as a stylesheet, and the wallpaper is shown as an image, where SVG cannot run scripts; the CSP blocks remote stylesheets, fonts and images, and rendered artifacts are served with a `sandbox` CSP. Free-text fields that reach CSS or SVG are validated too: theme names allow letters, digits and simple punctuation only. Without that rule, a tier 0 plan, which applies without asking, could have put an `@import` into `theme.css` or a script into `wallpaper.svg`.

## 9. Audit log

`crates/fluxd/src/audit.rs`: JSON lines, each entry holding the SHA-256 of the previous one (genesis is 64 zeros). Entries record prompt, operations, decision, plan hash, approver, generation and result. Verification runs at every boot and on `/v1/audit/verify`; editing any line breaks the chain from that entry on. Every append is also emitted on the `fluxd::audit` tracing target for journald, syslog and SIEM collection. There is no API that deletes entries.

## 10. Mapping to the PRD

| PRD item | Where |
|---|---|
| Preview before applying | `GET /v1/plans/{id}/preview`, `ui/src/state/system.ts` (`previewPlan`) |
| UI and shell (themes, animations, dock, panels, snapping, tiling) | renderers for the real desktop; `ui/src/desktop/` for the web shell |
| UI as demo and admin console (React, TypeScript, Vite, Tailwind, Zustand) | `ui/` |
| Declarative over imperative, whitelisted actions | `spec.rs`, `registry.rs`, `ops.rs` |
| Permission tiers and proposals in fleets | `policy.rs` (`decide`), `auth.rs` |
| Explain every change | `diff.rs` summaries, plan records, audit |
| Snapshot, verify, automatic rollback | `engine.rs` apply, `targets/`, `health.rs` |
| Kernel catalog, sysctl tuning, sched_ext | `catalog.rs`, NixOS renderer |
| Restricted-change windows | `policy.rs` `FreezeWindow` |
| Design tokens in DTCG, exporters per toolkit | `tokens.rs`, `render/theme.rs` |
| Hardware compatibility check | `hwprobe.rs`, `validate_spec` |
| MCP-compatible tool interface | `mcp.rs` |
| Signed profiles | `signing.rs`, export/import endpoints |
| Local-model option, rule-based fast path, caching | `agent/src/providers/ollama.ts`, `heuristic.ts`, `sessions.ts` |
| Industry profiles | `profiles/` |

Differences from the PRD worth knowing:

- nixpkgs 26.05 removed `linux_hardened`. The `hardened` kernel variant is the LTS kernel plus KSPP-style boot parameters and sysctls.
- The PRD lists Landlock, seccomp and microVMs for sandboxing generated code. FluxOS never generates code or scripts from model output, so the sandbox stage is the sandbox target plus `nixos-rebuild build`. The agent process itself runs under systemd sandboxing.
- eBPF/nftables policy, WASM plugins, fleet management, image signing pipeline, marketplace, billing and SSO are later phases.
