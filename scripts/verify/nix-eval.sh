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
  out=$(nixc eval --impure --json --expr "let r = import $ROOT/scripts/verify/eval.nix { module = $2; nixpkgs = $PKGS_EXPR; root = $ROOT; }; in { inherit (r) drv kernel scx session; warnings = r.warnings; }" 2>"$WORK/$1.err")
  if [ -z "$out" ]; then
    check "$1 evaluates" "$(grep -m1 'error:' -A3 "$WORK/$1.err" | tr '\n' ' ')" "a system derivation"
    return
  fi
  check_match "$1 evaluates ($(echo "$out" | jq -r '"kernel \(.kernel), scheduler \(.scx), \(.session)"'))" "$(echo "$out" | jq -r .drv)" '^/nix/store/.*-nixos-system-.*\.drv$'
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
  packages.x86_64-linux.appliance-ova \
  nixosConfigurations.fluxos-appliance.config.system.build.toplevel \
  nixosConfigurations.fluxos-vm.config.system.build.toplevel nixosConfigurations.fluxos-vm.config.system.build.vm; do
  out=$(nixc eval --raw "$FLAKE#$attr.drvPath" 2>"$WORK/flake.err")
  check_match "$attr evaluates" "${out:-$(grep -m1 'error:' "$WORK/flake.err")}" '^/nix/store/.*\.drv$'
done
for host in fluxos-appliance fluxos-vm; do
  check "$host has no NixOS warnings" "$(nixc eval --json "$FLAKE#nixosConfigurations.$host.config.warnings" 2>/dev/null | jq length)" 0
done
VM="$FLAKE#nixosConfigurations.fluxos-appliance.config"
opt() { # <option path> : its value as text (store paths without their string context)
  # No eval cache: Nix 2.31 trips over cached strings with derivation context.
  nixc eval --no-eval-cache --raw "$VM.$1" --apply 'v: builtins.unsafeDiscardStringContext (if builtins.isString v || builtins.isPath v || v ? outPath then toString v else builtins.toJSON v)' 2>&1 | grep -v '^warning:'
}
check_match "appliance: fluxd serves the UI package" "$(opt systemd.services.fluxd.serviceConfig.ExecStart)" '--ui-dir /nix/store/[a-z0-9]+-flux-ui-0\.1\.0( |$)'
check_match "appliance: fluxd rebuilds its own configuration" "$(opt systemd.services.fluxd.serviceConfig.ExecStart)" '--nixos-flake path:/etc/nixos#fluxos-appliance( |$)'
check_match "appliance: agent accepts the UI origin" "$(opt systemd.services.flux-agent.environment.FLUX_CORS_ORIGINS)" '(^|,)http://127\.0\.0\.1:7700(,|$)'
check "appliance: the FluxOS shell runs as flux" "$(opt services.cage.user)" flux
check_match "appliance: the shell is a full-screen browser on the UI" "$(opt services.cage.program)" '^/nix/store/[a-z0-9]+-fluxos-shell$'
check "appliance: the console screen is paired automatically" "$(opt systemd.paths.fluxos-console-pairing.pathConfig.PathChanged)" /var/lib/fluxos/secrets/pairing.code
check "appliance: fluxd adopts the image's spec" "$(opt '"environment"."etc"."fluxos-image/spec.json".enable')" true
check "appliance: /etc/fluxos is the live theme directory" "$(opt environment.etc.fluxos.source)" /run/fluxos/current/fluxos
check "appliance: guest support for VirtualBox and VMware" "$(opt virtualisation.virtualbox.guest.enable)/$(opt virtualisation.vmware.guest.enable)" "true/true"
check "appliance: the self-test only runs with its credential" "$(opt systemd.services.fluxos-selftest.unitConfig.ConditionCredential)" fluxos.selftest

summary
