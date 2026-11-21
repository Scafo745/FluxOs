# MorphOS: PRD and Tech Stack

Draft v0.1 · November 11, 2026 · Prepared for Rishabh

> Transcribed from the original PDF so the requirements live next to the code. The content is unchanged; this repository builds the product under the name FluxOS (see [status.md](status.md) for decisions and progress).

> Working name. An existing, unrelated operating system is already called MorphOS, so a rebrand and trademark check is on the to-do list (see Open Questions).

## 1. Summary

MorphOS is an AI-driven operating system builder. A person or a company describes what they need in plain language, and an AI agent produces a working, customized Linux-based system: kernel tuning, desktop environment, apps, security policy, and a matching visual design (fonts, colors, icons, animations).

The same engine serves many industries. A gamer, a school, a telecom operations center, a trading desk, and a hospital each get a different system from the same product, built from a reusable **profile**.

The product is built on one rule: **the AI has wide reach, but every action is validated, reversible, and logged.**

## 2. Problem

- Operating systems are static. Everyone gets the same defaults regardless of what they actually do.
- Customizing Linux deeply is possible but demands expert knowledge of configs, themes, kernels, and security hardening.
- Companies in regulated or specialized industries spend heavily on building and maintaining standardized, locked-down workstation images by hand.
- Existing AI assistants can answer questions about a system but rarely change it safely.

## 3. Vision and Goals

**Vision:** describe the computer you need, and get it.

**Goals**

1. Turn a natural-language request into a validated, working system configuration.
2. Make every change reversible so users can experiment without fear.
3. Generate matching visual design (typography, color, icons, motion) from a brand or an industry.
4. Let businesses produce signed, auditable OS images per department or industry.
5. Expand by adding industry profiles, not by rebuilding the product.

**Non-goals (for now)**

- Writing an operating system kernel from scratch.
- Copying macOS or Windows code, assets, or trademarks. We recreate look and behavior only, with original assets and neutral names.
- Supporting every hardware configuration on day one.
- Replacing the vendor apps an industry relies on (trading terminals, clinical systems). We integrate them.

## 4. Target Users

| Segment | Need | Priority |
|---|---|---|
| Enthusiasts and gamers | Total customization without manual config work | First (community and demo) |
| Schools and training centers | Locked-down, resettable, easy-to-manage machines | Early enterprise |
| Telecom and network operations | Dense dashboards, tuned networking, strict access control | Early enterprise |
| Finance and trading firms | Low latency, auditability, compliance posture | Later (higher compliance load) |
| Healthcare organizations | Locked-down kiosks, session security, accessibility | Later (highest compliance load) |
| Developers | Reproducible workstations from a prompt | Opportunistic |

## 5. Product Principles

1. **Reversible by default.** Nothing is applied without a snapshot or rollback path.
2. **Declarative over imperative.** The AI produces a description of the desired system, not a pile of commands.
3. **Whitelisted actions.** The agent chooses from validated operations. It does not run arbitrary commands on the host.
4. **Explain every change.** Each action stores the prompt, the plan, and the reason.
5. **Human control scales with risk.** Low-risk changes are automatic, high-risk changes need approval.
6. **Profiles, not one-offs.** Every result can be saved, shared, versioned, and reused.

## 6. Core Concept: The Profile Compiler

A **profile** is a structured, versioned bundle that fully describes a system:

- Base image and kernel selection
- Runtime kernel tuning (sysctl values, CPU governor, scheduler)
- Packages and apps
- Services and startup behavior
- Security and access policies
- Design tokens (colors, type, spacing, motion, icons)
- Desktop layout and window behavior

The **profile compiler** turns a request into a profile:

1. **Intake.** The user or company describes their needs, industry, apps, compliance needs, hardware, and brand.
2. **Retrieve.** The system pulls matching industry templates and knowledge.
3. **Generate.** The AI drafts a profile against a strict schema.
4. **Validate.** Schema checks, policy checks, and a hardware compatibility check.
5. **Preview.** The change is built in a sandbox or VM so the user can see it.
6. **Approve and apply.** Based on the permission tier.
7. **Verify.** Health checks run, with automatic rollback on failure.
8. **Publish.** Save the profile, or build a signed image for a fleet.

## 7. Functional Requirements

### 7.1 Natural-language customization

- Accept prompts such as "make it feel like macOS", "prioritize gaming above everything", or "set this up for a telecom operations desk".
- Ask clarifying questions only when the answer changes the outcome.
- Show a plain-language summary of the planned changes before applying anything above the lowest risk tier.

### 7.2 Adaptation layers

| Layer | What the AI can change | Mechanism |
|---|---|---|
| UI and shell | Themes, animations, dock, panels, window snapping, tiling | Desktop configs, extensions, compositor settings |
| Apps and services | Install, remove, configure software and services | Package managers, systemd, containers |
| Kernel | Pick from tested kernels, tune runtime parameters, swap schedulers | Kernel profiles, sysctl, sched_ext |
| Policy | Access rules, hardening, network rules | Landlock, AppArmor/SELinux, nftables, eBPF |
| Design | Fonts, colors, icons, motion | Design tokens exported to each toolkit |

### 7.3 Look and behavior recreation

- Provide original "styles" that recreate the feel of other systems: floating glass docks, snap layouts, tiling modes, global menu bars, animation curves.
- Use original icons, wallpapers, and fonts, and neutral names (for example "glass dock", "snap layouts").
- Legal review before launch for any style that approaches protected trade dress.

### 7.4 Kernel strategy

The AI never writes kernel code. It selects and tunes.

- **Tested kernel catalog:** a small set of vetted kernels (general, low-latency, hardened, long-term-support). Start with four or five.
- **Runtime tuning:** sysctl values, CPU governors, I/O schedulers, cgroup limits.
- **Hot-swappable schedulers:** use sched_ext to load scheduling policies tuned to the workload (gaming, low-latency, server) without a reboot. Validate current maturity before relying on it.
- **Fallback:** every kernel change keeps the previous boot entry, and a failed health check reverts automatically.
- **Later (opt-in, advanced):** Rust kernel modules, only after VM testing.

### 7.3.1 Design system generation

- Output design tokens in the W3C Design Tokens (DTCG) JSON format.
- Convert tokens into formats for web/CSS, GTK, Qt, and the chosen desktop's theme system.
- Fonts come from open-licensed families by default. Company brand fonts are supplied by the company as licensed files.
- Import brand colors and type from design tools where the tools can export tokens.
- Note: native toolkit theming does not reach every app (GTK, Qt, and Electron apps behave differently), so exporters must cover each.

### 7.5 Agent behavior

The agent runs a loop: understand, plan, act, verify, commit or roll back.

- Tools are registered in a tool registry with a declared risk tier and permission scope.
- Tool exposure is default-deny. Anything new is explicitly allowlisted.
- Tool inputs and outputs are treated as untrusted. Content the agent reads (web pages, files, tool descriptions) cannot trigger high-risk actions on its own.
- Permissions are re-checked on every tool call, not cached.

### 7.6 Permission tiers

| Tier | Examples | Behavior |
|---|---|---|
| 0: Safe | Wallpaper, theme, fonts, icons, animations | Auto-apply with snapshot |
| 1: Low | User-level apps, shell extensions, shortcuts | Auto-apply, undo offered |
| 2: Medium | System packages, services, firewall rules, scheduler swap | Preview and confirm |
| 3: High | Kernel switch, bootloader, disk, encryption, security policy | Explicit confirmation, sandbox test, rollback entry required |
| 4: Blocked | Wiping disks, disabling the firewall, removing audit logs | Never allowed by the agent |

In enterprise fleets, tiers 2 and above become **proposals** that an administrator approves, and rollouts run in scheduled windows.

### 7.7 Safety and recovery

- Snapshot or new system generation before every change.
- Declarative apply so the resulting state is known.
- Sandbox first for generated scripts and code (see Tech Stack).
- Automatic health checks (boots, network, display, key services) with rollback on failure.
- Append-only audit log: prompt, plan, actions, result, and who approved.

### 7.8 Enterprise features

- Build signed OS images per department or industry.
- Central policy and profile management, staged rollouts, and update control (download and stage, apply later).
- Role-based access and SSO.
- Audit export to logging and security tools.
- Support for restricted-change windows (for example, no updates during trading hours).

### 7.9 Compatibility

- Native Linux apps from standard repositories and Flatpak.
- Windows apps through compatibility layers or VMs where needed.
- Web and remote-session access for vendor tools that have no Linux build.
- Hardware compatibility check before kernel, driver, or boot changes (GPU drivers, secure boot, Wi-Fi chips).

## 8. Industry Profiles (Basic First)

Each vertical ships as a profile plus two or three light integrations. Deep integrations come later, driven by paying customers.

| Vertical | Basic starter set |
|---|---|
| Gaming | Steam, Lutris, Heroic, Proton/Wine, GPU drivers, game mode, low-latency scheduler, controller support, bold high-contrast themes |
| Education | Browser, office suite, video meeting tools, learning-platform access, coding tools, exam lock-down mode, clean state on every login, friendly themes |
| Telecom | SSH and terminal tools, packet analysis, SNMP and NETCONF clients, monitoring dashboards, SIP/VoIP tools, tuned networking, dense dark operations-center layouts |
| Finance | Browser-based trading platforms, market-data and charting apps, precise time sync, low-latency kernel profile, strict endpoint security, audit logging, calm multi-monitor layouts |
| Healthcare | Browser-based record systems, secure remote desktop, badge login, auto-lock, no local patient data, accessible large-text design |

**Recommended launch order:** Gaming, then Education, then Telecom, then Finance and Healthcare. Finance and healthcare need compliance review (for example HIPAA for US healthcare, and securities-regulator and record-keeping rules for finance), which should be defined by each customer's compliance team.

**Focus rule:** launch with two verticals (Gaming plus one enterprise vertical), not five.

## 9. Non-Functional Requirements

- **Reliability:** a failed change must never leave a machine unbootable. Rollback is mandatory.
- **Security:** least privilege everywhere, signed images, no arbitrary shell from the model, and prompt-injection resistance.
- **Privacy:** local-model option so prompts and system data can stay on-device.
- **Performance:** local rule-based fast path for common requests, with the LLM used for novel ones.
- **Auditability:** every action traceable to a prompt and an approver.
- **Portability:** an adapter layer so the engine can target more than one base distribution later.
- **Accessibility:** profiles must support high contrast, large text, and screen readers.

## 10. Success Metrics

- Share of prompts that produce a valid, applied configuration on the first attempt
- Rollback rate and rollback success rate (target: successful rollback every time)
- Time from prompt to working system
- Zero unrecoverable (unbootable) incidents
- Profiles created and shared per active user
- Enterprise pilots started and converted
- Latency p99 improvement for performance-sensitive profiles (gaming, finance, telecom), measured against a baseline

## 11. Competitive Landscape

- **AgntOS:** an open-source NixOS-based project where an LLM agent edits declarative configs with validation, audit, and rollback. Closest in concept.
- **Omarchy:** an Arch-based distribution reported to ship natural-language configuration features.
- **Community NixOS and AI tooling:** agent-assisted config editing is an active experiment.
- **Manual tooling:** NixOS, immutable distributions, and Ansible-style fleet tools, which are powerful but expert-only.

**Our differentiation:** industry profiles, brand-matched design generation, enterprise fleet and compliance features, and a strict safety model (tiers, sandbox, audit). We should verify competitor traction before pitching.

## 12. Business Model

1. **Freemium consumer.** A free tier with limits and local-model use. A paid tier for unlimited customization, cloud AI, profile sync, and backup.
2. **Profile marketplace.** Creators publish profiles (a gaming setup, a themed desktop). The platform takes a share of paid profiles.
3. **Enterprise subscription (main revenue engine).** Per-seat or per-device pricing for managed fleets, signed images, policy control, audit exports, and support.
4. **Hardware partnerships.** Pre-installation and licensing with small OEMs or integrators.
5. **Developer API.** The prompt-to-profile engine offered to other distributions and tools.

Pricing is not set in this draft. It should be tested with early pilots.

**Key business risks:** high AI inference costs (mitigate with local models and caching), a small initial desktop-Linux market, and customer trust that the AI cannot break their machines.

## 13. Roadmap

| Phase | Focus | Outcome |
|---|---|---|
| 0: Foundation | Keep the existing web MVP as the demo and marketing asset. Define the profile schema, tool registry, and permission tiers. | Clear schema and a working demo |
| 1: Real Linux MVP | Agent plus applier on a real Linux base, snapshot and rollback, themes and design tokens, gaming profile | A real system changed safely by a prompt |
| 2: Safety and profiles | Sandbox layer, audit log, profile sharing, education profile, hardware checks | Trustworthy and shareable |
| 3: Images and enterprise | Signed image builder, fleet management, telecom profile, admin approval flows | First enterprise pilots |
| 4: Regulated verticals | Finance and healthcare profiles with compliance review | Regulated customers |
| 5: Platform | Marketplace, WASM plugin ecosystem, developer API, hardware partners | Scaling |

Timelines depend on team size, which is not yet defined.

## 14. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| AI produces a bad config | Schema validation, sandbox preview, snapshots, automatic rollback |
| Prompt injection through web pages, files, or tool output | Treat all external content as untrusted, default-deny tools, tiered approvals |
| Malicious or poisoned plugins and tools | Signed plugins, WASM sandboxing, allowlists, review process |
| Unbootable system after kernel or driver change | Hardware compatibility checks, previous boot entry kept, health-check rollback |
| Legal issues around look-and-feel | Original assets, neutral names, legal review |
| Compliance gaps in regulated verticals | Customer compliance teams define rules, expert review before launch |
| Scope creep across five verticals | Launch two, expand by demand |
| Name collision with existing MorphOS | Rebrand and trademark search early |
| High inference cost | Local models, caching, rule-based fast path |

## 15. Tech Stack

### 15.1 Guiding approach

- **Rust** for anything privileged, always-on, or security-critical.
- **Python or TypeScript** for fast-changing AI orchestration and UI.
- **Declarative systems (Nix, container-based images)** for the OS definition itself.
- Use existing, mature desktop and kernel mechanisms instead of inventing new ones.

### 15.2 Stack by layer

| Layer | Choice | Why |
|---|---|---|
| Base OS | NixOS for v1 (declarative, built-in rollback). Fedora Atomic or Universal Blue images (bootc) as the enterprise image path. | Reproducible, reversible, and image-based |
| Alternative bases | openSUSE Aeon, Vanilla OS; Arch or Ubuntu with Btrfs snapshots | Reach and hardware support later via an adapter layer |
| Desktop environments | KDE Plasma and GNOME first; Hyprland for tiling; evaluate COSMIC (Rust) | Deeply themeable and scriptable |
| Window and animation behavior | Compositor configs first; Smithay (Rust) for a custom compositor later | Custom snapping, tiling, motion |
| Kernel | Catalog of tested kernels (general, low-latency, hardened, LTS) plus sysctl and sched_ext schedulers | Safe kernel-level adaptation |
| Policy | Landlock, AppArmor or SELinux, nftables, eBPF (Aya in Rust) | Verified, kernel-enforced rules |
| Agent runtime and daemon | Rust (async) | Memory-safe, small, privileged |
| Policy engine and validator | Rust | The trust boundary for every action |
| Applier and rollback manager | Rust, driving Nix, bootc, and snapshots | Deterministic apply |
| Tool registry and protocol | MCP-compatible tool interface with risk tiers and default-deny | Standard, extensible, controllable |
| AI orchestration | Python or TypeScript, multi-provider with failover | Fast iteration, rich libraries |
| Models | Cloud LLM providers plus local models (llama.cpp, Ollama) | Capability plus privacy and offline use |
| Knowledge and retrieval | Vector database for industry templates and docs | Domain-aware generation |
| Sandboxing | Landlock and seccomp for local actions; microVMs (Firecracker or Kata) or gVisor for generated code | Defense in depth |
| Plugins and connectors | WebAssembly components (wasmtime, WASI 0.2 now, 0.3 later) | Safe, language-agnostic extensions |
| Design tokens | W3C DTCG JSON; Style Dictionary or Terrazzo for conversion | Standard, exportable to every toolkit |
| Typography | Open-licensed fonts; cosmic-text for custom Rust rendering | Consistent, legally safe |
| UI (demo and admin console) | React and TypeScript (existing MVP), optionally Tauri for desktop; Leptos or Dioxus if going all-Rust | Reuse the current codebase |
| Image build pipeline | CI building signed OCI/bootc images and ISOs | Per-company OS images |
| Fleet management | Ansible or a custom Rust agent; SSO via Keycloak or similar | Enterprise control |
| Logging and audit | Append-only logs exported to syslog and SIEM tools | Compliance |
| Backend services | Rust or Go services, PostgreSQL, object storage for images | Profile storage, marketplace |
| Billing | Stripe | Subscriptions and marketplace payouts |
| Telemetry | Opt-in error and usage tracking | Product learning without privacy harm |

### 15.3 Agent architecture

Prompt, then the profile compiler, then the policy engine, then sandbox preview, then approval by tier, then the applier (with snapshot), then health checks, then commit or rollback, then the audit log.

- The model proposes a typed change plan against a strict schema.
- The policy engine rejects anything outside the allowlist before it reaches the system.
- The applier is deterministic and never executes free-form model output.

### 15.4 Existing MVP

The current web prototype (React, TypeScript, Vite, Tailwind, Zustand, with a local heuristic engine plus cloud AI failover) stays as the demo and as the front end for the admin console. Its in-memory process table, file system, and security module are simulations and are replaced by the real Linux backend in Phase 1.

## 16. Security Model Summary

- Model output is never trusted. It is validated against schemas and policy.
- External content (web pages, files, tool descriptions) can influence the model, so it cannot directly authorize high-risk actions.
- Tools are default-deny with explicit scopes, re-checked on every call.
- Local tool servers run sandboxed, not with full user permissions.
- Signed images and signed plugins, with an audit trail for every change.

## 17. Open Questions

1. **Brand:** a new name, plus a trademark search, since MorphOS already exists as a separate operating system.
2. **Team and runway:** how many people, and what timeline, so the roadmap can get dates.
3. **First enterprise vertical:** Education or Telecom (telecom matches the founder's networking background).
4. **Base choice for v1:** NixOS only, or NixOS plus a Fedora Atomic image path from the start.
5. **Desktop base:** KDE or GNOME, or the Rust-based COSMIC, as the first target.
6. **Local versus cloud AI mix:** and the per-user inference cost budget.
7. **Legal review:** scope of the look-and-feel recreation and any trade dress concerns.
8. **Verification items:** current maturity of sched_ext schedulers, bootc production readiness, and competitor traction.
9. **Compliance partners:** who advises on healthcare and finance requirements before those verticals begin.
