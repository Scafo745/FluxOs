#!/usr/bin/env bash
# Verifies fluxd's safety model end to end against a throwaway state
# directory: tiers and approvals, blocked actions, validation, staged kernel
# changes, automatic rollback, undo, MCP, signed profiles, audit chain, SSE,
# crash recovery, audit tamper detection, fleet mode and change freezes.
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

summary
