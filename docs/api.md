# FluxOS API reference

Two local services. Both bind to `127.0.0.1` by default and allow CORS from the origins in `FLUX_CORS_ORIGINS` (defaults include `http://localhost:5173` for a Vite dev server; the agent also allows `http://127.0.0.1:7700` and `http://localhost:7700`, where fluxd serves the UI).

- **fluxd** `http://127.0.0.1:7700`: system state, plans, approvals, generations, audit, events, pairing, and the web UI itself (see [Serving the web UI](#serving-the-web-ui)).
- **flux-agent** `http://127.0.0.1:7701`: natural-language intents.

Errors from both have the shape `{"error": {"code": "...", "message": "...", "details": ...}}`. Unknown `/v1/...` paths on fluxd return a JSON 404 (`not_found`), also when it serves the UI.

## Authentication

| Credential | Use it for |
|---|---|
| paired session `flxs_…` | a browser screen. Returned by `POST /v1/pair`; acts as the device owner (personal mode) or as a local user (fleet mode); revocable. fluxd stores only its SHA-256 in `secrets/sessions.json` |
| `secrets/admin.token` | the device owner's CLI and automation (`scripts/flux`): approvals, rollback, profiles, reboot |
| `secrets/agent.token` | flux-agent only |
| `secrets/intent.token` | clients calling flux-agent `/v1/intents`; paired screens receive it in the pairing response |
| `secrets/user.token` | fleet mode only: a non-admin person |

Send `Authorization: Bearer <token>`. `EventSource` cannot set headers, so `GET /v1/events?access_token=<token>` is accepted too; the web UI streams events with `fetch` and the header instead, which keeps tokens out of URLs and logs.

Principal ids name who acted, in plans and the audit log: `owner` and `local-user` for the static tokens, `owner@se_…` or `local-user@se_…` for a paired screen, `flux-agent` for the agent.

## fluxd

### Public (no token)

| Method | Path | Returns |
|---|---|---|
| GET | `/v1/health` | `{status: ok|degraded, version, boot_id, generation, target, mode, uptime_s, degraded[], ready, event_seq}` |
| GET | `/v1/boot` | boot report: `{version, boot_id, started_at, ready, duration_ms, target, generation, stages[{id, title, status: ok|warn|fail, detail, duration_ms}]}` |
| GET | `/v1/ui-config` | `{version, boot_id, mode, agent_url, pairing: "code"}` for a UI that is not paired yet |
| POST | `/v1/pair` | exchanges a one-time code for a session token, see [Pairing](#pairing) |
| GET | `/v1/theme.css` | current theme as CSS custom properties (`--flux-color-accent`, `--flux-font-sans`, `--flux-radius-md`, `--flux-duration-base`, `--flux-easing`, ...) |
| GET | `/v1/tokens.json` | current DTCG tokens (`application/design-tokens+json`) |
| GET | `/v1/wallpaper.svg` | current generated wallpaper |

`boot_id` changes every time fluxd starts. Rendered artifacts (`theme.css`, `tokens.json`, `wallpaper.svg`, and the `file` endpoints of plans and generations) are sent with `X-Content-Type-Options: nosniff` and `Content-Security-Policy: sandbox; default-src 'none'; style-src 'unsafe-inline'; img-src data:`, so opening one directly cannot run scripts.

### Pairing

fluxd only takes orders from screens a person paired. At every start it creates an 8-character code (Crockford base32, shown as `K7QM-3XRA`), prints it to its log (`fluxd::pairing`) and writes it to `<state>/secrets/pairing.code`. No API endpoint returns it, so a web page cannot pair itself. `fluxd pair-code` prints it (`sudo fluxd pair-code` on NixOS; `scripts/flux pair` in development).

```http
POST /v1/pair
{"code": "k7qm3xra", "name": "Firefox on Linux"}
```

Case, dashes, spaces and the look-alikes O/0 and I/L/1 are forgiven. The response:

```json
{
  "token": "flxs_…",
  "session": { "id": "se_…", "name": "Firefox on Linux", "role": "admin", "created_at": "…" },
  "principal": { "id": "owner@se_…", "role": "admin", "session": "se_…" },
  "mode": "personal",
  "intent_token": "…",
  "agent_url": "http://127.0.0.1:7701"
}
```

A code works once; after a successful pairing fluxd writes the next one. A wrong code returns 403 `wrong_code` with `details.attempts_left`. The fifth wrong try rotates the code and pauses pairing for 30 seconds: 429 `pairing_locked` with `details.retry_after_s`. Successful pairings and lockouts are audited (`session.paired`, `pairing.locked`).

### Sessions

| Method | Path | Notes |
|---|---|---|
| POST | `/v1/logout` | revokes the caller's own session (static tokens get 400) |
| GET | `/v1/sessions` | admin only: `{current, sessions[{id, name, role, created_at}]}` |
| POST | `/v1/sessions/{id}/revoke` | your own session, or any session as an admin |

A revoked token stops working at once; the UI that held it returns to the pairing screen when it sees the `session.revoked` event or a 401.

### Events (SSE)

`GET /v1/events` replays up to 512 buffered events, then streams live ones. Each SSE message has `id` = sequence number, `event` = kind, `data` = `{seq, ts, kind, data}`. Reconnects send `Last-Event-ID` (or `?since=<seq>`) to resume. To get only new events, read `event_seq` from `/v1/health` and subscribe with `?since=<event_seq>`.

Sequence numbers restart when fluxd restarts. A cursor beyond the newest event therefore comes from an earlier daemon instance, and fluxd replays the new instance's buffer from its first event, which is `boot.begin` with the new `boot_id`. Compare it with the `boot_id` you saw before to detect a restart.

| Kind | Data |
|---|---|
| `boot.begin` | `{version, target, boot_id}`; `{…, reboot: true}` when a simulated reboot starts |
| `boot.stage` | a stage object; during a simulated reboot the stages are `firmware`, `bootloader`, `kernel`, `scheduler`, `session` and a real `health` run, each with `reboot: true` |
| `boot.ready` | `{generation, duration_ms, target, boot_id, degraded[]}`; `reboot: true` after a simulated reboot |
| `plan.proposed`, `plan.blocked`, `plan.invalid`, `plan.noop` | `{plan_id, tier, decision, summary, status, prompt, source, session_id}` |
| `plan.approved`, `plan.rejected` | `{plan_id, by?}` |
| `apply.started` | `{plan_id, generation, previous, tier, origin}` |
| `apply.step` | `{plan_id, generation, step}` |
| `apply.health` | `{plan_id, generation, check: {id, name, status, detail}}` |
| `apply.rolled_back`, `apply.failed` | `{plan_id, generation, restored, failed[] | error, origin}` |
| `generation.switched` | `{generation, previous, plan_id, staged, theme, summary, origin}` |
| `system.reboot` | `{generation, message, staged_activated}` |
| `session.paired`, `session.revoked` | `{session: {id, name, role, created_at}}` |

`origin` is `{source, session_id}` from the plan's intent. The web UI uses one id per browser tab, both as `intent.session_id` on its own plans and as the agent `session_id`; the agent copies its session id into the plans it proposes. So a UI can tell its own changes from changes made elsewhere and notify only about the latter.

### State

| Method | Path | Notes |
|---|---|---|
| GET | `/v1/whoami` | `{id, role, session, mode, can_approve_tiers[]}`; `session` is null for static tokens |
| GET | `/v1/system` | `{generation, status, theme_name, spec_digest, spec, target, mode, hardware, degraded, pending_approvals, audit_head}` |
| GET | `/v1/hardware` | probe result (or `<state>/hardware.json` override) |
| GET | `/v1/tools` | registry with tier, behavior, scope, `params_schema` (JSON Schema) |
| GET | `/v1/catalog` | packages, services, fonts, icon and cursor themes, kernels, schedulers, sysctls, splash themes |
| GET | `/v1/schema` | JSON Schemas of plan requests and the full spec |

### Plans

Plan request body (also used by `/v1/plans/validate`):

```json
{
  "intent": { "prompt": "make it purple", "source": "ui", "session_id": "optional" },
  "summary": "optional, shown as the AI's summary",
  "rationale": "optional",
  "engine": { "kind": "ui" },
  "operations": [ { "tool": "design.generate_palette", "params": { "accent": "#a855f7" }, "reason": "optional" } ]
}
```

| Method | Path | Notes |
|---|---|---|
| POST | `/v1/plans?auto_apply=true` | validate and record; with `auto_apply`, tier 0-1 plans apply immediately |
| POST | `/v1/plans/validate` | dry run: same response, nothing stored or audited |
| GET | `/v1/plans?status=proposed&limit=50` | newest first |
| GET | `/v1/plans/{id}` | full plan record |
| GET | `/v1/plans/{id}/preview` | candidate spec and artifact list; `?path=etc/fluxos/theme.css` returns one rendered file (theme, wallpaper, NixOS module) before approval |
| POST | `/v1/plans/{id}/approve` | body `{plan_hash, apply?: true}`; needs a role allowed for the tier |
| POST | `/v1/plans/{id}/apply` | tier 0-1 by anyone; tier 2-3 only by a person after approval |
| POST | `/v1/plans/{id}/reject` | body `{reason?}` |

Plan record fields the UI needs:

```json
{
  "id": "pl_…", "status": "proposed|approved|applied|staged|rolled_back|failed|blocked|invalid|no_op|rejected",
  "decision": "auto_apply|confirm|admin_approval|blocked",
  "plan_hash": "sha256:…",
  "change_summary": "plain-language lines derived from the validated diff",
  "ai_summary": "the model's own words (untrusted)",
  "evaluation": {
    "ok": true, "blocked": false, "tier": 3, "requires_reboot": true,
    "changes": [ { "path": "kernel.variant", "before": "lts", "after": "low-latency", "tier": 3, "summary": "Kernel: …", "requires_reboot": true } ],
    "issues": [ { "severity": "error|warning", "code": "unknown_package", "path": "…", "message": "…", "suggestions": ["steam"] } ]
  },
  "preview": { "ok": true, "artifacts": [ … ], "checks": [ … ] },
  "result": { "generation": 4, "status": "applied|staged|rolled_back|failed", "message": "…", "health": [ … ], "rolled_back": false }
}
```

Approval flow for the UI: show `change_summary` and `evaluation.changes` (sorted riskiest first), then `POST /v1/plans/{id}/approve` with the `plan_hash` you displayed. If the plan changed, the hash no longer matches and fluxd answers 409.

Status codes worth handling: 401 bad token, 403 role not allowed, 409 stale or expired plan or hash mismatch, 423 change freeze active, 503 daemon degraded (tier 2+ refused).

### Generations, rollback, reboot

| Method | Path | Notes |
|---|---|---|
| GET | `/v1/generations` | `{current, generations[{number, parent, status, summary, tier, theme_name, requires_reboot, artifacts}]}` |
| GET | `/v1/generations/{n}` | `{meta, spec}` |
| GET | `/v1/generations/{n}/file?path=…` | a rendered artifact of that generation |
| POST | `/v1/rollback` | body `{to?, prompt?, session_id?}`; default is the parent of the current generation. Applied right away when the caller may approve the resulting tier. To show the risk first, send a `generation.restore` operation to `/v1/plans/validate`: the response has the diff and the tier |
| POST | `/v1/system/reboot` | admin only. Sandbox: simulated reboot that activates a staged generation and replays boot stages. NixOS: `systemctl reboot` |

### Profiles

| Method | Path | Notes |
|---|---|---|
| GET | `/v1/profiles` | summaries |
| GET | `/v1/profiles/{id}` | summary, description and materialized spec |
| POST | `/v1/profiles` | `{id, name, summary?}` saves the current system as a profile |
| GET | `/v1/profiles/{id}/export` | Ed25519-signed bundle |
| POST | `/v1/profiles/import` | bundle signed by a trusted key (own key or `<state>/trusted-keys/*.pub`) |

### Audit

| Method | Path | Notes |
|---|---|---|
| GET | `/v1/audit?limit=50&before=<seq>&kind=apply.` | newest first, plus the chain head |
| GET | `/v1/audit/verify` | `{ok, entries, head, broken_at?, error?}` |

### Serving the web UI

`fluxd serve --ui-dir ui/dist` serves the built UI at `/` on the API's origin; paths that are not files fall back to `index.html`. In development, `scripts/dev.sh` runs the Vite dev server instead, which proxies `/v1` to fluxd.

| Flag (environment) | Default | Meaning |
|---|---|---|
| `--ui-dir` (`FLUX_UI_DIR`) | none | directory with the built UI; fluxd refuses to start if it has no `index.html` |
| `--agent-url` (`FLUX_AGENT_PUBLIC_URL`) | `http://127.0.0.1:7701` | flux-agent as the browser reaches it; returned by `/v1/ui-config` and `/v1/pair`, and allowed in the CSP |
| `--allowed-hosts` (`FLUX_ALLOWED_HOSTS`) | none | extra Host header names to accept, comma separated |

UI responses carry `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src 'self' <agent origin>; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`, plus `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY` and `Referrer-Policy: no-referrer`.

Every request, API or UI, passes a Host check first. `127.0.0.1`, `::1`, `localhost` and `*.localhost` are accepted, with any port, plus the names in `--allowed-hosts`. Anything else gets 421 `host_not_allowed`. fluxd listens on loopback, but a browser tricked by DNS rebinding would send the attacker's hostname; this check stops such pages from reaching the API.

### MCP

`POST /mcp` speaks JSON-RPC 2.0 (MCP Streamable HTTP with JSON responses): `initialize`, `ping`, `tools/list`, `tools/call`. Tool names use underscores (`design_generate_palette`). Each call becomes a one-operation plan with the same policy: tier 0-1 applies, tier 2-3 returns the plan id for human approval, tier 4 is refused.

### Developer

`POST /v1/dev/fault` with `{"fault": "health" | "apply" | null}` (requires `--dev` and an admin token) makes the next apply fail its health check or activation, to demonstrate automatic rollback.

## flux-agent

| Method | Path | Notes |
|---|---|---|
| GET | `/v1/health` | fluxd link, routing mode, provider chain with circuit state and latency |
| POST | `/v1/intents` | run the pipeline and return the result |
| POST | `/v1/intents/stream` | same, as SSE: `step` events while it works, then one `result` event |
| GET | `/v1/sessions/{id}` | conversation turns and pending questions |

Request:

```json
{ "prompt": "set this up for my school", "session_id": "optional", "answers": { "shared_lab": "Shared lab: wipe the student account on every boot" }, "auto_apply": true, "engine": "auto|local|llm" }
```

Response:

```json
{
  "id": "in_…", "session_id": "ss_…",
  "status": "applied|staged|awaiting_approval|needs_clarification|answered|blocked|invalid|no_op|rolled_back|failed|error",
  "message": "what to show the user",
  "engine": { "kind": "heuristic|llm|cache|rollback", "provider": "anthropic", "model": "claude-sonnet-5-5", "coverage": 0.83 },
  "questions": [ { "id": "shared_lab", "question": "…", "why": "…", "options": ["…"] } ],
  "plan": { "id": "pl_…", "tier": 3, "decision": "confirm", "plan_hash": "sha256:…", "change_summary": "…", "changes": [ … ], "requires_reboot": true, "issues": [ … ], "approve_url": "http://127.0.0.1:7700/v1/plans/pl_…/approve" },
  "result": { "generation": 5, "status": "applied", "message": "…", "health": { "passed": 8, "total": 8, "failed": [] } },
  "trace": [ { "stage": "route", "detail": "local rule engine (understood 100% of the request)", "ms": 12 } ]
}
```

Trace stages, in order: `intake`, `context`, `retrieve`, `route`, `provider` (one per attempt), `generate`, `clarify`, `validate`, `repair`, `propose`, `verify`, `rollback`, `done`. They are designed to drive a live "AI thinking" view in the UI.

For `needs_clarification`, send the same prompt again with `answers` keyed by question id and the same `session_id`.
