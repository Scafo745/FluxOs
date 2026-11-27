#!/usr/bin/env bash
# Renders generation 1 of the FluxOS appliance (the default profile) into
# nix/hosts/fluxos: generated.nix, imported by the appliance configuration,
# and spec.json, which fluxd adopts on first boot. Run it after changing the
# default spec or the NixOS renderer.
#
#   cargo build -p fluxd && scripts/render-host.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FLUXD="${FLUXD_BIN:-$ROOT/target/debug/fluxd}"
out=$(mktemp -d)
trap 'rm -rf "$out"' EXIT
"$FLUXD" render --profile "${1:-default}" --out "$out" --generation 1 >/dev/null
cp "$out/etc/nixos/fluxos/generated.nix" "$ROOT/nix/hosts/fluxos/generated.nix"
cp "$out/etc/fluxos/spec.json" "$ROOT/nix/hosts/fluxos/spec.json"
echo "nix/hosts/fluxos/generated.nix and spec.json now hold generation 1 of the ${1:-default} profile"
