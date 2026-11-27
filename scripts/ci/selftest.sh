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
say() { echo "FLUXOS-SELFTEST $*"; }
finish() {
  say "journal (warnings and errors):"
  journalctl -b -p warning --no-pager -o short-monotonic | tail -n 80 | sed 's/^/  | /'
  say "fluxd log:"
  journalctl -b -u fluxd --no-pager -o short-monotonic | tail -n 60 | sed 's/^/  | /'
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
