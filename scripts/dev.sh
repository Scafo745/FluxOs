#!/usr/bin/env bash
# Boots FluxOS locally: fluxd (sandbox target, safe on any Linux host),
# flux-agent and the web UI. Ctrl-C stops everything.
#
#   scripts/dev.sh                 UI from the Vite dev server (hot reload) on :5173
#   FLUX_UI=dist scripts/dev.sh    build the UI once; fluxd serves it on :7700
#   FLUX_UI=none scripts/dev.sh    backend only
#
# AI providers are optional. Without them the local rule engine handles
# common requests. Put keys in agent/.env (see agent/.env.example).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="${FLUX_STATE_DIR:-$ROOT/.flux-state}"
UI_MODE="${FLUX_UI:-vite}"
cd "$ROOT"

case "$UI_MODE" in
  vite | dist | none) ;;
  *) echo "FLUX_UI must be vite, dist or none (got '$UI_MODE')" >&2; exit 2 ;;
esac

echo "› building fluxd"
cargo build -q -p fluxd
echo "› building flux-agent"
(cd agent && { [ -d node_modules ] || npm install --no-audit --no-fund --silent; } && npm run -s build)
if [ "$UI_MODE" != none ]; then
  echo "› preparing the web UI"
  (cd ui && { [ -d node_modules ] || npm install --no-audit --no-fund --silent; })
fi
UI_ARGS=()
if [ "$UI_MODE" = dist ]; then
  echo "› building the web UI"
  (cd ui && npm run -s build >/dev/null)
  UI_ARGS=(--ui-dir "$ROOT/ui/dist")
fi

# fluxd writes a fresh one-time pairing code on every start.
rm -f "$STATE/secrets/pairing.code"
./target/debug/fluxd serve --state-dir "$STATE" --listen 127.0.0.1:7700 --dev ${UI_ARGS[@]+"${UI_ARGS[@]}"} &
FLUXD=$!
AGENT=""
VITE=""
trap 'kill $FLUXD $AGENT $VITE 2>/dev/null || true' EXIT INT TERM

for _ in $(seq 1 100); do [ -f "$STATE/secrets/intent.token" ] && [ -f "$STATE/secrets/pairing.code" ] && break; sleep 0.1; done

if [ -f agent/.env ]; then set -a; . agent/.env; set +a; fi
export FLUX_AGENT_TOKEN_FILE="$STATE/secrets/agent.token"
export FLUX_INTENT_TOKEN_FILE="$STATE/secrets/intent.token"
node agent/dist/main.js &
AGENT=$!

if [ "$UI_MODE" = vite ]; then
  (cd ui && exec ./node_modules/.bin/vite --clearScreen false --logLevel warn) &
  VITE=$!
fi
sleep 1.5

CODE="$(cat "$STATE/secrets/pairing.code" 2>/dev/null || echo "")"
case "$UI_MODE" in
  vite) UI_URL="http://127.0.0.1:5173" ;;
  dist) UI_URL="http://127.0.0.1:7700" ;;
  *) UI_URL="" ;;
esac

echo
echo "FluxOS is running"
echo "  fluxd       http://127.0.0.1:7700   boot log: curl -s localhost:7700/v1/boot | jq"
echo "  flux-agent  http://127.0.0.1:7701"
[ -n "$UI_URL" ] && echo "  web UI      $UI_URL"
echo "  state       $STATE"
if [ -n "$UI_URL" ] && [ -n "$CODE" ]; then
  cat <<EOF

Open this link to pair the browser (the code works once):
  $UI_URL/#pair=${CODE//-/}
Or type the code on the pairing screen: $CODE
To pair another screen later: scripts/flux pair
EOF
fi
cat <<EOF

Talk to it with scripts/flux, for example:
  scripts/flux say "make it feel like macOS"
  scripts/flux say "prioritize gaming above everything"
  scripts/flux pending
  scripts/flux approve <plan-id>
  scripts/flux undo
EOF
wait
