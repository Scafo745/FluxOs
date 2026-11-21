#!/usr/bin/env bash
# Evaluates what FluxOS generates as complete NixOS systems (no build):
#   1. every built-in profile, rendered by `fluxd render`
#   2. edge cases driven through a live fluxd (exam kiosk, COSMIC, Plasma
#      and GNOME tiling, NVIDIA open driver, hardened kernel, many services)
#   3. the flake: packages and the bootable fluxos-vm system
#
#   cargo build -p fluxd && scripts/verify/nix-eval.sh
#
# Environment:
#   NIX      nix command line (default "nix"), e.g. "nix --store /some/store"
#   NIXPKGS  local nixpkgs checkout; default is the revision pinned in flake.lock
source "$(dirname "$0")/lib.sh"
need curl jq git
need_fluxd
NIX=${NIX:-nix}
command -v "${NIX%% *}" >/dev/null 2>&1 || { echo "nix not found (set NIX)" >&2; exit 2; }
nixc() { $NIX --extra-experimental-features 'nix-command flakes' "$@"; }

FLAKE="git+file://$ROOT"
if [ -n "${NIXPKGS:-}" ]; then
  PKGS_EXPR="$NIXPKGS"
else
  PKGS_EXPR="(builtins.getFlake \"$FLAKE\").inputs.nixpkgs.outPath"
fi
WORK=$(mktemp -d "${TMPDIR:-/tmp}/flux-nix-eval.XXXXXX")

# eval_module <label> <path to generated.nix>
eval_module() {
  local out
  out=$(nixc eval --impure --json --expr "let r = import $ROOT/scripts/verify/eval.nix { module = $2; nixpkgs = $PKGS_EXPR; }; in { inherit (r) drv kernel scx; warnings = r.warnings; }" 2>"$WORK/$1.err")
  if [ -z "$out" ]; then
    check "$1 evaluates" "$(grep -m1 'error:' -A3 "$WORK/$1.err" | tr '\n' ' ')" "a system derivation"
    return
  fi
  check_match "$1 evaluates ($(echo "$out" | jq -r '"kernel \(.kernel), scheduler \(.scx)"'))" "$(echo "$out" | jq -r .drv)" '^/nix/store/.*-nixos-system-.*\.drv$'
  check "$1 has no NixOS warnings" "$(echo "$out" | jq '.warnings | length')" 0
}

section "Built-in profiles"
for p in default gaming education telecom finance healthcare; do
  "$FLUXD_BIN" render --profile "$p" --out "$WORK/$p" --generation 2 >/dev/null
  eval_module "profile $p" "$WORK/$p/etc/nixos/fluxos/generated.nix"
done

section "Edge cases through fluxd"
STATE="$WORK/state"
mkdir -p "$STATE"
write_hardware "$STATE" nvidia 0x10de
PORT=17730
start_fluxd "$STATE" $PORT
A=$(cat "$STATE/secrets/admin.token")
U=http://127.0.0.1:$PORT
edge() { # <label> <operations json>
  local p r g
  p=$(curl -s -X POST -H "Authorization: Bearer $A" -H 'content-type: application/json' "$U/v1/plans" -d "{\"operations\":$2}")
  if [ "$(echo "$p" | jq -r .status)" != proposed ]; then
    check "$1 plan validates" "$(echo "$p" | jq -c '[.evaluation.issues[] | select(.severity=="error") | .message]')" proposed
    return
  fi
  r=$(curl -s -X POST -H "Authorization: Bearer $A" -H 'content-type: application/json' "$U/v1/plans/$(echo "$p" | jq -r .id)/approve" -d "{\"plan_hash\":\"$(echo "$p" | jq -r .plan_hash)\"}")
  g=$(echo "$r" | jq -r .result.generation)
  curl -s -H "Authorization: Bearer $A" "$U/v1/generations/$g/file?path=etc/nixos/fluxos/generated.nix" >"$WORK/edge-$1.nix"
  eval_module "edge $1" "$WORK/edge-$1.nix"
}
edge exam '[{"tool":"kernel.select","params":{"variant":"hardened"}},{"tool":"drivers.configure","params":{"gpu":"nvidia-open","gaming_controllers":true}},{"tool":"kernel.tune","params":{"io_scheduler":"bfq","cpu_governor":"performance","sysctl":{"net.ipv4.tcp_rmem":"4096 87380 16777216"}}},{"tool":"accessibility.set","params":{"screen_reader":true,"high_contrast":true}},{"tool":"security.configure","params":{"usb_storage":"block"}},{"tool":"session.configure","params":{"mode":"exam","kiosk_url":"https://exam.example.edu/test?id=42"}},{"tool":"boot.configure","params":{"splash_theme":"spinner"}}]'
edge cosmic '[{"tool":"session.configure","params":{"mode":"standard"}},{"tool":"desktop.set_environment","params":{"environment":"cosmic"}},{"tool":"kernel.select","params":{"variant":"performance"}},{"tool":"kernel.set_scheduler","params":{"scheduler":"bpfland"}},{"tool":"services.enable","params":{"services":["ssh","tailscale","docker","libvirtd","printing","avahi","smartcard","chrony"]}},{"tool":"firewall.allow","params":{"tcp":[8080],"udp":[51820]}}]'
edge plasma-tiling '[{"tool":"desktop.set_environment","params":{"environment":"plasma"}},{"tool":"desktop.apply_style","params":{"style":"tiling"}},{"tool":"packages.install","params":{"packages":["wireshark","netconf-client","vscode","zoom","discord","slack","geogebra"]}},{"tool":"session.configure","params":{"ephemeral_home":true}}]'
edge gnome-tiling '[{"tool":"desktop.set_environment","params":{"environment":"gnome"}},{"tool":"desktop.set_windows","params":{"tiling":true,"blur":true}},{"tool":"desktop.set_layout","params":{"dock_enabled":true,"dock_position":"left"}},{"tool":"accessibility.set","params":{"large_text":true,"text_scale":1.5}}]'
stop "$FLUXD_PID"

section "Flake (nixpkgs pinned by flake.lock)"
for attr in packages.x86_64-linux.fluxd packages.x86_64-linux.flux-agent packages.x86_64-linux.flux-ui \
  nixosConfigurations.fluxos-vm.config.system.build.toplevel nixosConfigurations.fluxos-vm.config.system.build.vm; do
  out=$(nixc eval --raw "$FLAKE#$attr.drvPath" 2>"$WORK/flake.err")
  check_match "$attr evaluates" "${out:-$(grep -m1 'error:' "$WORK/flake.err")}" '^/nix/store/.*\.drv$'
done
check "fluxos-vm has no NixOS warnings" "$(nixc eval --json "$FLAKE#nixosConfigurations.fluxos-vm.config.warnings" 2>/dev/null | jq length)" 0
VM="$FLAKE#nixosConfigurations.fluxos-vm.config"
check_match "fluxos-vm: fluxd serves the UI package" "$(nixc eval --raw "$VM.systemd.services.fluxd.serviceConfig.ExecStart" 2>&1)" '--ui-dir /nix/store/[a-z0-9]+-flux-ui-0\.1\.0( |$)'
check_match "fluxos-vm: agent accepts the UI origin" "$(nixc eval --raw "$VM.systemd.services.flux-agent.environment.FLUX_CORS_ORIGINS" 2>&1)" '(^|,)http://127\.0\.0\.1:7700(,|$)'
check "fluxos-vm: UI opens at login" "$(nixc eval --raw "$VM.environment.etc.\"xdg/autostart/fluxos.desktop\".text" 2>/dev/null | grep -c '^Exec=/nix/store/.*-fluxos-open-ui$')" 1

summary
