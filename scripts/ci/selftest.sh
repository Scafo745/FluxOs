# FluxOS appliance self-test. Runs inside the VM as root (systemd unit
# fluxos-selftest in nix/hosts/appliance.nix), only when the VM was started
# with the SMBIOS credential fluxos.selftest. Every line it prints goes to the
# serial port; scripts/ci/boot-test.sh reads them and takes screenshots at the
# markers. It never spends the pairing code, which belongs to the shell.
# NixOS runs unit scripts with `set -e`; every failure here is reported instead.
set +e -u
U=http://127.0.0.1:7700
AGENT=http://127.0.0.1:7701
S=/var/lib/fluxos/secrets
# Each report opens the serial port anew: the serial getty that systemd starts
# on it hangs up file descriptors opened before it.
TTY=${SELFTEST_TTY:-/dev/ttyS0}
say() { echo "FLUXOS-SELFTEST $*" >>"$TTY"; }
dump() { sed 's/^/  | /' >>"$TTY"; }
finish() {
  say "journal (warnings and errors):"
  journalctl -b -p warning --no-pager -o short-monotonic | tail -n 80 | dump
  say "fluxd log:"
  journalctl -b -u fluxd --no-pager -o short-monotonic | tail -n 60 | dump
  say "shell log (cage, browser):"
  journalctl -b -u cage-tty1 --no-pager -o short-monotonic | tail -n 40 | dump
  say "flux-session log:"
  journalctl -b _SYSTEMD_USER_UNIT=flux-session.service --no-pager -o short-monotonic | tail -n 20 | dump
  say "DONE $1"
  exit 0
}
fail() { say "FAIL $*"; finish fail; }
api() { curl -sS --max-time "${T:-30}" -H "Authorization: Bearer $(cat $S/admin.token)" -H 'content-type: application/json' "$@"; }

for _ in $(seq 1 120); do curl -sf -o /dev/null $U/v1/health && break; sleep 1; done
health=$(curl -sf $U/v1/health) || fail "fluxd did not answer"
say "health $(echo "$health" | jq -c '{status, target, generation, ready, degraded}')"
curl -sf $U/v1/boot | jq -r '.stages[] | "stage \(.id) \(.status): \(.detail)"' | while read -r l; do say "$l"; done
[ "$(echo "$health" | jq -r .target)" = nixos ] || fail "fluxd is not on the nixos target"
curl -sf $U/ | grep -q '<div id="root">' || fail "the UI is not served"
say "ui served"

for _ in $(seq 1 90); do systemctl -q is-active cage-tty1.service && break; sleep 1; done
say "session cage-tty1 $(systemctl is-active cage-tty1.service)"
# The shell pairs itself with the link fluxos-console-pairing hands over.
paired=0
for _ in $(seq 1 90); do
  paired=$(api $U/v1/sessions | jq '.sessions | length')
  [ "${paired:-0}" -ge 1 ] && break
  sleep 2
done
say "paired screens $paired"
[ "${paired:-0}" -ge 1 ] || fail "the shell did not pair"

# The UI's terminal, files, processes and apps: flux-session runs in flux's
# user manager, and fluxd relays to it.
sess='{}'
for _ in $(seq 1 30); do
  sess=$(api $U/v1/session)
  [ "$(echo "$sess" | jq -r .available)" = true ] && break
  sleep 2
done
say "session $(echo "$sess" | jq -c '{available, user, shell, wayland, reason}')"
[ "$(echo "$sess" | jq -r .available)" = true ] || fail "fluxd cannot reach flux-session"
[ "$(echo "$sess" | jq -r .user)" = flux ] || fail "flux-session does not run as flux"
say "session files $(api "$U/v1/session/files?path=~" | jq -c '{path, entries: (.entries | length)}')"
say "session processes $(api $U/v1/session/processes | jq -c '{count: (.processes | length), mine: ([.processes[] | select(.user == "flux") | .name] | unique)}')"
say "session apps $(api $U/v1/session/apps | jq -c '[.apps[].id]')"
# The Terminal app's path end to end: a ticket, then the WebSocket (bash's
# /dev/tcp; client frames are masked with a zero key), a command in flux's
# login shell and its exit code.
ticket=$(api -X POST $U/v1/session/terminal -d '{"cols":100,"rows":30}' | jq -r '.ticket // empty')
[ "${ticket#flxt_}" != "$ticket" ] || fail "no terminal ticket"
ws_send() { printf "\\x82\\x$(printf %02x $((0x80 | ${#1})))\\x00\\x00\\x00\\x00%s" "$1" >&3; }
term=""
if exec 3<>/dev/tcp/127.0.0.1/7700; then
  printf 'GET /v1/session/terminal?ticket=%s HTTP/1.1\r\nHost: 127.0.0.1:7700\r\nOrigin: http://127.0.0.1:7700\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n' "$ticket" >&3
  sleep 2
  ws_send 'echo FLUX-TERM-$((6*7)) $(id -un) $SHELL'$'\r'
  sleep 2
  ws_send $'exit\r'
  term=$(timeout 10 cat <&3 | tr -c '[:print:]\n' '.')
  exec 3>&-
fi
say "terminal $(echo "$term" | head -n 1 | tr -d '\r') / $(echo "$term" | grep -ao 'FLUX-TERM-42 [^.]*' | head -n 1) / $(echo "$term" | grep -ao '"code":[0-9]*' | head -n 1)"
echo "$term" | grep -aq 'FLUX-TERM-42 flux' || fail "the terminal did not run a command as flux"
echo "$term" | grep -aq '"code":0' || fail "the terminal did not report the shell's exit"
sleep 12
say "screenshot desktop"

# Tier 0 through the AI: the agent plans it, fluxd validates and applies it.
# Only live files change, so no nixos-rebuild runs.
res=$(curl -sS --max-time 120 -X POST $AGENT/v1/intents -H "Authorization: Bearer $(cat $S/intent.token)" \
  -H 'content-type: application/json' -d '{"prompt":"neon green and pink theme"}')
say "intent $(echo "$res" | jq -c '{status, generation: .result.generation, message: (.message | split("\n")[0])}')"
gen=$(echo "$res" | jq -r '.result.generation // empty')
[ -n "$gen" ] || fail "the theme change was not applied"
[ "$(cat /etc/fluxos/generation 2>/dev/null)" = "$gen" ] || fail "/etc/fluxos does not serve generation $gen"
grep -q 'Neon' /etc/fluxos/theme.css && say "live theme $(grep -m1 -o 'Neon[A-Za-z ]*' /etc/fluxos/theme.css)"
sleep 10
say "screenshot theme"

# Tier 2, approved by the owner: a real nixos-rebuild build + switch.
plan=$(api -X POST $U/v1/plans -d '{"intent":{"prompt":"install htop","source":"selftest"},"operations":[{"tool":"packages.install","params":{"packages":["htop"]}}]}')
id=$(echo "$plan" | jq -r .id)
say "plan $(echo "$plan" | jq -c '{id, tier: .evaluation.tier, status, decision}')"
start=$(date +%s)
r=$(T=3000 api -X POST "$U/v1/plans/$id/approve" -d "{\"plan_hash\":$(echo "$plan" | jq .plan_hash)}")
say "apply $(echo "$r" | jq -c '{status, generation: .result.generation, message: .result.message, seconds: '"$(( $(date +%s) - start ))"'}')"
say "apply steps $(echo "$r" | jq -c '.result.steps')"
[ "$(echo "$r" | jq -r .status)" = applied ] || fail "nixos-rebuild apply did not succeed"
[ -x /run/current-system/sw/bin/htop ] || fail "htop is not in the new system"
say "htop $(readlink -f /run/current-system/sw/bin/htop)"
say "health after apply $(echo "$r" | jq -c '[.result.health[] | "\(.id):\(.status)"]')"

say "audit $(api $U/v1/audit/verify | jq -c '{ok, entries}')"
sleep 5
say "screenshot final"
finish ok
