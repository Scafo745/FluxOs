#!/usr/bin/env bash
# Verifies fluxd's safety model end to end against a throwaway state
# directory: screen pairing and browser-facing hardening, tiers and
# approvals, blocked actions, validation, staged kernel changes, automatic
# rollback, undo, MCP, signed profiles, audit chain, SSE, crash recovery,
# audit tamper detection, fleet mode, change freezes and serving the web UI.
#
#   cargo build -p fluxd && scripts/verify/daemon.sh
source "$(dirname "$0")/lib.sh"
need curl jq
need_fluxd

STATE=$(mktemp -d "${TMPDIR:-/tmp}/fluxd-verify.XXXXXX")
PORT=17700
U=http://127.0.0.1:$PORT
write_hardware "$STATE"
start_fluxd "$STATE" $PORT --dev
A=$(cat "$STATE/secrets/admin.token")
G=$(cat "$STATE/secrets/agent.token")

post() { curl -s -X POST -H "Authorization: Bearer $1" -H 'content-type: application/json' "$U$2" -d "$3"; }
get() { curl -s -H "Authorization: Bearer $1" "$U$2"; }
http_code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }
errors() { jq -r '[.evaluation.issues[] | select(.severity=="error") | .code] | unique | join(",")'; }

curl -s -N "$U/v1/events?access_token=$G" >"$STATE/events.sse" &
SSE=$!
PIDS+=("$SSE")

section "Boot"
check "health is ok" "$(curl -s $U/v1/health | jq -r .status)" ok
check "every boot stage is ok" "$(curl -s $U/v1/boot | jq '[.stages[] | select(.status != "ok")] | length')" 0
check "generation 1 bootstrapped" "$(get $G /v1/system | jq .generation)" 1
check "unauthenticated request refused" "$(http_code $U/v1/system)" 401
check "unknown token refused" "$(http_code -H 'Authorization: Bearer flx_wrong' $U/v1/system)" 401

section "Screen pairing"
pair() { curl -s -X POST -H 'content-type: application/json' "$U/v1/pair" -d "$1"; }
check "pairing code is not exposed by the API" "$(curl -s $U/v1/boot $U/v1/health $U/v1/ui-config | grep -c "$(tr -d '-' <"$STATE/secrets/pairing.code" | tr -d '\n')")" 0
check "ui-config tells the UI where the agent is" "$(curl -s $U/v1/ui-config | jq -r .agent_url)" "http://127.0.0.1:7701"
check "wrong code refused" "$(pair '{"code":"AAAA-AAAA"}' | jq -r .error.code)" wrong_code
CODE=$(cat "$STATE/secrets/pairing.code")
P=$(pair "{\"code\":\"$(echo "$CODE" | tr 'A-Z' 'a-z')\",\"name\":\"Verify browser\"}")
S=$(echo "$P" | jq -r .token)
check "code pairs (case and dash insensitive)" "$(echo "$P" | jq -r '.principal.role')" admin
check "pairing hands over the agent intent token" "$(echo "$P" | jq -r .intent_token)" "$(cat "$STATE/secrets/intent.token")"
check "session token works" "$(curl -s -H "Authorization: Bearer $S" $U/v1/whoami | jq -r '.id | startswith("owner@se_")')" true
check "codes are single use" "$(pair "{\"code\":\"$CODE\"}" | jq -r .error.code)" wrong_code
check "only the token hash is stored" "$(grep -c "$S" "$STATE/secrets/sessions.json")" 0
check "admin lists paired screens" "$(get $A /v1/sessions | jq '.sessions | length')" 1
check "agent cannot list paired screens" "$(get $G /v1/sessions | jq -r .error.code)" forbidden
for _ in 1 2 3 4; do pair '{"code":"AAAA-AAAA"}' >/dev/null; done
check "fifth wrong code locks pairing" "$(pair '{"code":"AAAA-AAAA"}' | jq -r .error.code)" pairing_locked
check "even the right code waits out the lockout" "$(pair "{\"code\":\"$(cat "$STATE/secrets/pairing.code")\"}" | jq -r .error.code)" pairing_locked
check "logout revokes the session" "$(curl -s -X POST -H "Authorization: Bearer $S" $U/v1/logout | jq -r .revoked.name)" "Verify browser"
check "revoked token refused" "$(http_code -H "Authorization: Bearer $S" $U/v1/whoami)" 401
check "static tokens cannot log out" "$(post $A /v1/logout '{}' | jq -r .error.code)" bad_request

section "Browser-facing hardening"
check "foreign Host refused (DNS rebinding)" "$(http_code -H 'Host: attacker.example' $U/v1/health)" 421
check "loopback Host names accepted" "$(http_code -H 'Host: localhost:5173' $U/v1/health)/$(http_code -H 'Host: [::1]:7700' $U/v1/health)" "200/200"
check "unknown API route is a JSON 404" "$(curl -s $U/v1/nope | jq -r .error.code)" not_found
check "artifacts are served sandboxed" "$(curl -s -D - -o /dev/null $U/v1/wallpaper.svg | grep -ci '^content-security-policy: sandbox')" 1
check "theme names cannot carry CSS or markup" \
  "$(post $G /v1/plans/validate '{"operations":[{"tool":"design.set_palette","params":{"theme_name":"x */ @import url(//e.example/a.css); /*"}}]}' | errors)" invalid_theme_name
check "theme names cannot carry SVG tags" \
  "$(post $G /v1/plans/validate '{"operations":[{"tool":"design.set_palette","params":{"theme_name":"</title><script>"}}]}' | errors)" invalid_theme_name
check "health reports the event cursor" "$(curl -s $U/v1/health | jq '.event_seq > 0')" true
STALE=$(curl -s -N -m 2 -H "Authorization: Bearer $G" "$U/v1/events?since=999999" | grep -m1 '^event:' || true)
check "a cursor from an older daemon replays this boot" "$STALE" "event: boot.begin"

section "Tier 0: the agent applies with a snapshot"
R=$(post $G '/v1/plans?auto_apply=true' '{"intent":{"prompt":"pink neon","source":"verify"},"operations":[{"tool":"design.generate_palette","params":{"accent":"#ff2e97","scheme":"dark","theme_name":"Pink Neon"}}]}')
check "applied" "$(echo "$R" | jq -r .status)" applied
check "as generation 2" "$(echo "$R" | jq -r .result.generation)" 2
check "all health checks passed" "$(echo "$R" | jq '[.result.health[] | select(.status=="fail")] | length')" 0
check "theme.css serves the new accent" "$(curl -s $U/v1/theme.css | grep -c -- '--flux-color-accent: #ff2e97')" 1

section "Tier 2: needs a person, bound to the plan hash"
P=$(post $G /v1/plans '{"intent":{"prompt":"install steam"},"operations":[{"tool":"packages.install","params":{"packages":["steam"]}}]}')
ID=$(echo "$P" | jq -r .id)
HASH=$(echo "$P" | jq -r .plan_hash)
check "proposed, waiting for confirmation" "$(echo "$P" | jq -r '"\(.status)/\(.decision)/\(.evaluation.tier)"')" "proposed/confirm/2"
check "summary comes from the validated diff" "$(echo "$P" | jq -r .change_summary)" "System packages: +steam"
check "agent cannot apply it" "$(post $G /v1/plans/$ID/apply '{}' | jq -r .error.code)" forbidden
check "agent cannot approve it" "$(post $G /v1/plans/$ID/approve "{\"plan_hash\":\"$HASH\"}" | jq -r .error.code)" forbidden
check "approval with a different hash refused" "$(post $A /v1/plans/$ID/approve '{"plan_hash":"sha256:stale"}' | jq -r .error.code)" conflict
R=$(post $A /v1/plans/$ID/approve "{\"plan_hash\":\"$HASH\"}")
check "owner approval applies it" "$(echo "$R" | jq -r .status)" applied
check "approver recorded" "$(echo "$R" | jq -r .approval.by)" owner

section "Tier 4: never allowed"
for t in shell.exec disk.wipe firewall.disable audit.purge; do
  check "$t refused" "$(post $G /v1/plans "{\"operations\":[{\"tool\":\"$t\",\"params\":{}}]}" | jq -r '"\(.status)/\(.evaluation.tier)"')" "blocked/4"
done
check "audit off through security.configure refused by the diff" \
  "$(post $G /v1/plans '{"operations":[{"tool":"security.configure","params":{"audit":false}}]}' | jq -r '"\(.status)/\(.evaluation.tier)"')" "blocked/4"

section "Validation"
v() { post $G /v1/plans/validate "{\"operations\":[$1]}" | errors; }
check "unknown tool (default-deny)" "$(v '{"tool":"design.set_pallete","params":{}}')" unknown_tool
check "suggests the closest tool" "$(post $G /v1/plans/validate '{"operations":[{"tool":"design.set_pallete","params":{}}]}' | jq -r '.evaluation.issues[0].suggestions[0]')" design.set_palette
check "unknown parameter field" "$(v '{"tool":"design.set_palette","params":{"bogus":1}}')" invalid_params
check "package outside the catalog" "$(v '{"tool":"packages.install","params":{"packages":["steem"]}}')" unknown_package
check "unreadable contrast" "$(v '{"tool":"design.set_palette","params":{"text":"#222222","background":"#111111"}}')" low_contrast
check "sysctl outside the allowlist" "$(v '{"tool":"kernel.tune","params":{"sysctl":{"kernel.panic":1}}}')" invalid_sysctl
check "NVIDIA driver without an NVIDIA GPU" "$(v '{"tool":"drivers.configure","params":{"gpu":"nvidia"}}')" hardware_mismatch
check "kiosk URL with shell characters" "$(v '{"tool":"session.configure","params":{"mode":"kiosk","kiosk_url":"https://x.example/$(reboot)"}}')" invalid_kiosk_url
check "font outside the open-licensed catalog" "$(v '{"tool":"design.set_typography","params":{"font_sans":"Comic Sans MS"}}')" unknown_font

section "Tier 3: kernel switch is staged, previewed and approved"
P=$(post $G /v1/plans '{"intent":{"prompt":"prioritize gaming above everything"},"operations":[{"tool":"profile.apply","params":{"profile":"gaming"}}]}')
ID=$(echo "$P" | jq -r .id)
HASH=$(echo "$P" | jq -r .plan_hash)
check "tier 3, needs a reboot" "$(echo "$P" | jq -r '"\(.evaluation.tier)/\(.evaluation.requires_reboot)"')" "3/true"
check "riskiest change listed first" "$(echo "$P" | jq -r '.evaluation.changes[0].tier')" 3
check "preview renders the NixOS module before approval" \
  "$(get $A "/v1/plans/$ID/preview?path=etc/nixos/fluxos/generated.nix" | grep -c 'boot.kernelPackages = pkgs.linuxPackages_zen;')" 1
R=$(post $A /v1/plans/$ID/approve "{\"plan_hash\":\"$HASH\"}")
check "staged for the next boot" "$(echo "$R" | jq -r .status)" staged
check "agent cannot reboot" "$(post $G /v1/system/reboot '{}' | jq -r .error.code)" forbidden
check "simulated reboot activates it" "$(post $A /v1/system/reboot '{}' | jq -r .staged_activated)" true
check "agent cannot shut down" "$(post $G /v1/system/poweroff '{}' | jq -r .error.code)" forbidden
check "the sandbox has no machine to shut down" "$(post $A /v1/system/poweroff '{}' | jq -r .error.code)" not_supported

section "Automatic rollback"
CUR=$(get $G /v1/system | jq .generation)
check "fault injection is admin-only" "$(post $G /v1/dev/fault '{"fault":"health"}' | jq -r .error.code)" forbidden
post $A /v1/dev/fault '{"fault":"health"}' >/dev/null
R=$(post $G '/v1/plans?auto_apply=true' '{"operations":[{"tool":"desktop.set_wallpaper","params":{"style":"aurora","seed":99}}]}')
check "failed health check rolls back" "$(echo "$R" | jq -r .status)" rolled_back
check "previous generation still active" "$(get $G /v1/system | jq .generation)" "$CUR"
post $A /v1/dev/fault '{"fault":"apply"}' >/dev/null
R=$(post $G '/v1/plans?auto_apply=true' '{"operations":[{"tool":"desktop.set_wallpaper","params":{"style":"mesh","seed":7}}]}')
check "failed activation restores the previous generation" "$(echo "$R" | jq -r '"\(.status)/\(.result.rolled_back)"')" "failed/true"
check "previous generation still active" "$(get $G /v1/system | jq .generation)" "$CUR"

section "Undo"
R=$(post $A /v1/rollback '{}')
check "restore plan" "$(echo "$R" | jq -r .kind)" restore
check_match "returns to the parent generation" "$(echo "$R" | jq -r .status)" '^(applied|staged)$'
check "theme is back" "$(get $G /v1/system | jq -r .theme_name)" "Pink Neon"

section "MCP"
check "initialize" "$(post $G /mcp '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' | jq -r .result.serverInfo.name)" fluxd
check "blocked tools are not listed" "$(post $G /mcp '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | jq '[.result.tools[].name] | index("shell_exec")')" null
check "tier 0 call applies" "$(post $G /mcp '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"design_set_motion","arguments":{"preset":"calm"}}}' | jq -r .result.isError)" false
check "repeat call is a no-op" "$(post $G /mcp '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"design_set_motion","arguments":{"preset":"calm"}}}' | jq -r .result.content[0].text)" "No change needed: the system already matches."
check "tier 2 call waits for a person" "$(post $G /mcp '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"services_enable","arguments":{"services":["ssh"]}}}' | jq -r .result.content[0].text | grep -c 'needs approval')" 1

section "Signed profiles"
check "save the current system" "$(post $A /v1/profiles '{"id":"my-rig","name":"My Rig"}' | jq -r .source)" user
B=$(get $A /v1/profiles/my-rig/export)
check "export is signed" "$(echo "$B" | jq -r '.signature | length > 40')" true
check "tampered bundle refused" "$(post $A /v1/profiles/import "$(echo "$B" | jq -c '.profile.name = "Evil Rig"')" | jq -r .error.code)" forbidden
check "agent cannot import" "$(post $G /v1/profiles/import "$B" | jq -r .error.code)" forbidden
check "signed bundle imports" "$(post $A /v1/profiles/import "$B" | jq -r .source)" imported

section "Audit and events"
check "hash chain verifies" "$(get $A /v1/audit/verify | jq -r .ok)" true
check "refused attempts are audited" "$(get $A '/v1/audit?limit=500&kind=plan.blocked' | jq '.entries | length')" 5
check "approvals record the approver" "$(get $A '/v1/audit?limit=500&kind=plan.approved' | jq -r '.entries[-1].actor')" owner
stop "$SSE"
check "SSE replayed boot events" "$(grep -c '^event: boot.stage' "$STATE/events.sse" | awk '{print ($1 > 0) ? "yes" : "no"}')" yes
check "SSE streamed generation switches" "$(grep -c '^event: generation.switched' "$STATE/events.sse" | awk '{print ($1 > 0) ? "yes" : "no"}')" yes

section "Crash recovery"
stop "$FLUXD_PID"
CUR=$(cat "$STATE/current")
echo "{\"generation\":99,\"previous\":$CUR,\"plan_id\":\"pl_0\",\"started_at\":\"verify\"}" >"$STATE/inflight.json"
echo "tampered" >"$STATE/sandbox/root/etc/fluxos/theme.css"
start_fluxd "$STATE" $PORT --dev
check "interrupted apply rolled back at boot" "$(curl -s $U/v1/boot | jq -r '.stages[] | select(.id=="recovery") | .status')" warn
check "in-flight marker cleared" "$(test -e "$STATE/inflight.json" && echo present || echo cleared)" cleared
check "live files restored" "$(grep -c tampered "$STATE/sandbox/root/etc/fluxos/theme.css")" 0
check "boot health passes" "$(curl -s $U/v1/boot | jq -r '.stages[] | select(.id=="health") | .status')" ok

section "Audit tamper detection"
stop "$FLUXD_PID"
sed -i '3s/"summary":"[^"]*"/"summary":"edited"/' "$STATE/audit/audit.jsonl"
start_fluxd "$STATE" $PORT --dev
check "daemon degrades" "$(curl -s $U/v1/health | jq -r .status)" degraded
P=$(post $G /v1/plans '{"operations":[{"tool":"packages.install","params":{"packages":["vlc"]}}]}')
check "tier 2 refused while degraded" "$(post $A /v1/plans/$(echo "$P" | jq -r .id)/approve "{\"plan_hash\":\"$(echo "$P" | jq -r .plan_hash)\"}" | jq -r .error.code)" degraded
check "tier 0 still works" "$(post $G '/v1/plans?auto_apply=true' '{"operations":[{"tool":"design.set_motion","params":{"preset":"snappy"}}]}' | jq -r .status)" applied
stop "$FLUXD_PID"

section "Fleet mode and change freeze"
FLEET=$(mktemp -d "${TMPDIR:-/tmp}/fluxd-fleet.XXXXXX")
write_hardware "$FLEET"
cat >"$FLEET/policy.json" <<EOF
{ "mode": "fleet", "disabled_tools": ["desktop.set_environment"],
  "freeze_windows": [{ "name": "trading-hours", "days": [$(date -u +%u)], "start": "00:00", "end": "23:59", "utc_offset_minutes": 0, "min_tier": 2 }] }
EOF
FPORT=17720
start_fluxd "$FLEET" $FPORT
FA=$(cat "$FLEET/secrets/admin.token")
FU=$(cat "$FLEET/secrets/user.token")
fpost() { curl -s -X POST -H "Authorization: Bearer $1" -H 'content-type: application/json' "http://127.0.0.1:$FPORT$2" -d "$3"; }
check "users approve tiers 0-1 only" "$(curl -s -H "Authorization: Bearer $FU" http://127.0.0.1:$FPORT/v1/whoami | jq -c .can_approve_tiers)" "[0,1]"
P=$(fpost $FU /v1/plans '{"operations":[{"tool":"packages.install","params":{"packages":["vlc"]}}]}')
check "tier 2 becomes an admin proposal" "$(echo "$P" | jq -r .decision)" admin_approval
check "user cannot approve it" "$(fpost $FU /v1/plans/$(echo "$P" | jq -r .id)/approve "{\"plan_hash\":\"$(echo "$P" | jq -r .plan_hash)\"}" | jq -r .error.code)" forbidden
check "admin approval is deferred by the freeze" "$(fpost $FA /v1/plans/$(echo "$P" | jq -r .id)/approve "{\"plan_hash\":\"$(echo "$P" | jq -r .plan_hash)\"}" | jq -r .error.code)" change_freeze
check "tier 0 is not frozen" "$(fpost $FU '/v1/plans?auto_apply=true' '{"operations":[{"tool":"design.set_motion","params":{"preset":"calm"}}]}' | jq -r .status)" applied
check "operator-disabled tool refused" "$(fpost $FA /v1/plans/validate '{"operations":[{"tool":"desktop.set_environment","params":{"environment":"gnome"}}]}' | errors)" disabled_tool
check "fleet mode pairs screens as a local user, not an admin" \
  "$(curl -s -X POST -H 'content-type: application/json' "http://127.0.0.1:$FPORT/v1/pair" -d "{\"code\":\"$(cat "$FLEET/secrets/pairing.code")\"}" | jq -r .principal.role)" user

section "NixOS target: live theme files (dry run)"
NX=$(mktemp -d "${TMPDIR:-/tmp}/fluxd-nixos.XXXXXX")
write_hardware "$NX"
NPORT=17750
N=http://127.0.0.1:$NPORT
start_fluxd "$NX" $NPORT --target nixos --nixos-dry-run
NG=$(cat "$NX/secrets/agent.token")
NA=$(cat "$NX/secrets/admin.token")
npost() { curl -s -X POST -H "Authorization: Bearer $1" -H 'content-type: application/json' "$N$2" -d "$3"; }
LIVE="$NX/logs/live/current"
check "boot publishes generation 1's live files" "$(cat "$LIVE/fluxos/generation" 2>/dev/null)" 1
check "the default desktop is the FluxOS shell" "$(curl -s -H "Authorization: Bearer $NA" $N/v1/system | jq -r .spec.desktop.environment)" fluxos
R=$(npost $NG '/v1/plans?auto_apply=true' '{"operations":[{"tool":"design.generate_palette","params":{"accent":"#39ff14","theme_name":"Neon Green"}}]}')
check "first apply writes generated.nix and rebuilds" "$(echo "$R" | jq -r '[.result.steps[] | select(startswith("[dry-run] nixos-rebuild"))] | length')" 2
R=$(npost $NG '/v1/plans?auto_apply=true' '{"operations":[{"tool":"design.generate_palette","params":{"accent":"#ff2e97","theme_name":"Neon Pink"}}]}')
check "a palette change needs no rebuild" "$(echo "$R" | jq -r '.result.message')" "Generation 3 is live (no rebuild needed)"
check "live files switched to the new generation" "$(cat "$LIVE/fluxos/generation")/$(grep -c 'Neon Pink' "$LIVE/fluxos/theme.css")" "3/1"
check "live files pass the health check" "$(echo "$R" | jq -r '.result.health[] | select(.id=="live.files") | .status')" pass
check "the module on disk keeps the last rebuilt configuration" "$(grep -c 'fluxos-gen-2' "$NX/logs/dry-run-generated.nix")" 1
R=$(npost $NA /v1/rollback '{"to":2}')
check "going back to an earlier theme needs no rebuild" "$(echo "$R" | jq -r '[.result.steps[] | select(startswith("[dry-run] nixos-rebuild"))] | length')/$(cat "$LIVE/fluxos/generation")" "0/4"
P=$(npost $NG /v1/plans '{"operations":[{"tool":"packages.install","params":{"packages":["htop"]}}]}')
R=$(npost $NA "/v1/plans/$(echo "$P" | jq -r .id)/approve" "{\"plan_hash\":\"$(echo "$P" | jq -r .plan_hash)\"}")
check "a package install rebuilds" "$(echo "$R" | jq -r '[.result.steps[] | select(startswith("[dry-run] nixos-rebuild"))] | length')/$(grep -c 'pkgs.htop' "$NX/logs/dry-run-generated.nix")" "2/1"
check "shutting down runs systemctl poweroff" "$(npost $NA /v1/system/poweroff '{}' | jq -r .message)" "[dry-run] systemctl poweroff"
check "shutting down is audited" "$(curl -s -H "Authorization: Bearer $NA" "$N/v1/audit?kind=system.poweroff" | jq '.entries | length')" 1
stop "$FLUXD_PID"

section "User session relay (terminal, files, processes)"
SESSION_BIN="${SESSION_BIN:-$ROOT/target/debug/flux-session}"
if [ -x "$SESSION_BIN" ]; then
  SX=$(mktemp -d "${TMPDIR:-/tmp}/fluxd-session.XXXXXX")
  mkdir -p "$SX/state"
  write_hardware "$SX/state"
  "$SESSION_BIN" --socket "$SX/session.sock" 2>/dev/null &
  PIDS+=("$!")
  SPORT=17760
  SU=http://127.0.0.1:$SPORT
  start_fluxd "$SX/state" $SPORT --session-socket "$SX/session.sock"
  SA=$(cat "$SX/state/secrets/admin.token")
  SG=$(cat "$SX/state/secrets/agent.token")
  sget() { curl -s -H "Authorization: Bearer $1" "$SU$2"; }
  check "the session is reachable for the owner" "$(sget $SA /v1/session | jq -r .available)" true
  check "the AI agent cannot use the user's session" "$(sget $SG /v1/session/files | jq -r .error.code)" forbidden
  check "files are listed with the user's rights" "$(sget $SA "/v1/session/files?path=$ROOT/nix" | jq -r '[.entries[].name] | index("module.nix") != null')" true
  check "text files are read" "$(sget $SA "/v1/session/file?path=$ROOT/LICENSE" | jq -r '.text | startswith("MIT License")')" true
  check "processes come from /proc" "$(sget $SA /v1/session/processes | jq -r '.processes | length > 0')" true
  check "pid 1 cannot be signalled" "$(curl -s -X POST -H "Authorization: Bearer $SA" -H 'content-type: application/json' $SU/v1/session/processes/1/signal -d '{"signal":"KILL"}' | jq -r .error.code)" bad_request
  check "a terminal needs a ticket" "$(http_code "$SU/v1/session/terminal?ticket=flxt_forged" -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==')" 401
  if command -v node >/dev/null 2>&1; then
    TK=$(curl -s -X POST -H "Authorization: Bearer $SA" -H 'content-type: application/json' $SU/v1/session/terminal -d '{"cols":100,"rows":30}' | jq -r .ticket)
    cat >"$SX/ws.mjs" <<'EOF'
const url = process.argv[2];
const run = () => new Promise((resolve) => {
  const ws = new WebSocket(url);
  ws.binaryType = "arraybuffer";
  let out = "", exit = null, done = false;
  const finish = (opened) => { if (!done) { done = true; resolve({ out, exit, opened }); } };
  let opened = false;
  ws.onopen = () => { opened = true; ws.send(JSON.stringify({ type: "resize", cols: 132, rows: 43 })); ws.send(new TextEncoder().encode("stty size; exit 7\n")); };
  ws.onmessage = (m) => { if (typeof m.data === "string") { const v = JSON.parse(m.data); if (v.type === "exit") exit = v.code; } else out += new TextDecoder().decode(m.data); };
  ws.onclose = () => finish(opened);
  ws.onerror = () => finish(opened);
  setTimeout(() => finish(opened), 8000);
});
const first = await run();
const second = await run();
console.log(`${first.out.includes("43 132")}/${first.exit}/${second.opened}`);
EOF
    check "the terminal runs a shell, resizes and reports its exit code; tickets are single use" "$(node "$SX/ws.mjs" "ws://127.0.0.1:$SPORT/v1/session/terminal?ticket=$TK")" "true/7/false"
  fi
  check "opening a terminal is audited" "$(sget $SA '/v1/audit?kind=session.terminal' | jq '.entries | length > 0')" true
  stop "$FLUXD_PID"
else
  echo "  (skipped: build flux-session with cargo build -p flux-session)"
fi

section "Serving the web UI"
WEB=$(mktemp -d "${TMPDIR:-/tmp}/fluxd-ui.XXXXXX")
mkdir -p "$WEB/ui" "$WEB/state"
echo '<!doctype html><title>flux-ui-marker</title>' >"$WEB/ui/index.html"
write_hardware "$WEB/state"
WPORT=17740
W=http://127.0.0.1:$WPORT
start_fluxd "$WEB/state" $WPORT --ui-dir "$WEB/ui"
check "UI served at /" "$(curl -s $W/ | grep -c flux-ui-marker)" 1
check "client-side routes fall back to index.html" "$(curl -s $W/apps/timeline | grep -c flux-ui-marker)" 1
CSP=$(curl -s -D - -o /dev/null $W/ | tr -d '\r' | grep -i '^content-security-policy:')
check_match "UI CSP allows scripts from fluxd only" "$CSP" "script-src 'self';"
check_match "UI CSP allows network access to fluxd and the agent only" "$CSP" "connect-src 'self' http://127\.0\.0\.1:7701;"
check "UI cannot be framed" "$(curl -s -D - -o /dev/null $W/ | grep -ci '^x-frame-options: deny')" 1
check "API routes win over the UI fallback" "$(curl -s $W/v1/nope | jq -r .error.code)" not_found
check "foreign Host refused for the UI too" "$(http_code -H 'Host: attacker.example' $W/)" 421
mkdir -p "$WEB/empty"
timeout 10 "$FLUXD_BIN" serve --state-dir "$WEB/state2" --listen 127.0.0.1:17741 --ui-dir "$WEB/empty" >/dev/null 2>&1
check "a UI directory without index.html is refused at start" "$?" 1

summary
