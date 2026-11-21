#!/usr/bin/env bash
# Boots the FluxOS backend locally: fluxd (sandbox target, safe on any Linux
# host) and flux-agent. Ctrl-C stops both.
#
# AI providers are optional. Without them the local rule engine handles
# common requests. Put keys in agent/.env (see agent/.env.example).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="${FLUX_STATE_DIR:-$ROOT/.flux-state}"
cd "$ROOT"

echo "› building fluxd"
cargo build -q -p fluxd
echo "› building flux-agent"
(cd agent && { [ -d node_modules ] || npm install --no-audit --no-fund --silent; } && npm run -s build)

./target/debug/fluxd serve --state-dir "$STATE" --listen 127.0.0.1:7700 --dev &
FLUXD=$!
AGENT=""
trap 'kill $FLUXD $AGENT 2>/dev/null || true' EXIT INT TERM

for _ in $(seq 1 100); do [ -f "$STATE/secrets/intent.token" ] && break; sleep 0.1; done

if [ -f agent/.env ]; then set -a; . agent/.env; set +a; fi
export FLUX_AGENT_TOKEN_FILE="$STATE/secrets/agent.token"
export FLUX_INTENT_TOKEN_FILE="$STATE/secrets/intent.token"
node agent/dist/main.js &
AGENT=$!
sleep 1

cat <<EOF

FluxOS backend is running
  fluxd       http://127.0.0.1:7700   boot log: curl -s localhost:7700/v1/boot | jq
  flux-agent  http://127.0.0.1:7701
  state       $STATE

Talk to it with scripts/flux, for example:
  scripts/flux say "make it feel like macOS"
  scripts/flux say "prioritize gaming above everything"
  scripts/flux pending
  scripts/flux approve <plan-id>
  scripts/flux undo
EOF
wait
