# FluxOS API reference

Two local services. Both bind to `127.0.0.1` by default and allow CORS from the origins in `FLUX_CORS_ORIGINS` (defaults include `http://localhost:5173` for a Vite dev server).

- **fluxd** `http://127.0.0.1:7700`: system state, plans, approvals, generations, audit, events.
- **flux-agent** `http://127.0.0.1:7701`: natural-language intents.

Errors from both have the shape `{"error": {"code": "...", "message": "...", "details": ...}}`.

## Authentication

| Token file (`<state>/secrets/`) | Use it for |
|---|---|
| `admin.token` | the web UI acting for the device owner: approvals, rollback, profiles, reboot |
| `agent.token` | flux-agent only |
| `intent.token` | clients calling flux-agent `/v1/intents` |
| `user.token` | fleet mode only: a non-admin person |

Send `Authorization: Bearer <token>`. `EventSource` cannot set headers, so `GET /v1/events?access_token=<token>` is accepted too.

## fluxd

### Public (no token)

| Method | Path | Returns |
|---|---|---|
| GET | `/v1/health` | `{status: ok|degraded, version, generation, target, mode, uptime_s, degraded[], ready}` |
| GET | `/v1/boot` | boot report: `{version, started_at, ready, duration_ms, target, generation, stages[{id, title, status: ok|warn|fail, detail, duration_ms}]}` |
| GET | `/v1/theme.css` | current theme as CSS custom properties (`--flux-color-accent`, `--flux-font-sans`, `--flux-radius-md`, `--flux-duration-base`, `--flux-easing`, ...) |
| GET | `/v1/tokens.json` | current DTCG tokens (`application/design-tokens+json`) |
| GET | `/v1/wallpaper.svg` | current generated wallpaper |

### Events (SSE)

`GET /v1/events` replays up to 512 buffered events, then streams live ones. Each SSE message has `id` = sequence number, `event` = kind, `data` = `{seq, ts, kind, data}`. Reconnects send `Last-Event-ID` (or `?since=<seq>`) to resume.

| Kind | Data |
|---|---|
| `boot.begin`, `boot.stage`, `boot.ready` | boot progress (`boot.stage` data is a stage object; `reboot: true` during a simulated reboot) |
| `plan.proposed`, `plan.blocked`, `plan.invalid`, `plan.noop` | `{plan_id, tier, decision, summary, status}` |
| `plan.approved`, `plan.rejected` | `{plan_id, by?}` |
| `apply.started` | `{plan_id, generation, previous, tier}` |
| `apply.step` | `{plan_id, generation, step}` |
| `apply.health` | `{plan_id, generation, check: {id, name, status, detail}}` |
| `apply.rolled_back`, `apply.failed` | `{plan_id, generation, restored, failed[] | error}` |
| `generation.switched` | `{generation, previous, plan_id, staged, theme, summary}` |
| `system.reboot` | `{generation, message, staged_activated}` |

### State

| Method | Path | Notes |
|---|---|---|
| GET | `/v1/whoami` | `{id, role, mode, can_approve_tiers[]}` |
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
| POST | `/v1/rollback` | body `{to?, prompt?}`; default is the parent of the current generation. Applied right away when the caller may approve the resulting tier |
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
