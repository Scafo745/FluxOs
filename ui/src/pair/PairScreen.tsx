import { KeyRound, ShieldCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Aurora } from "../boot/BootView";
import { FluxMark } from "../components/FluxMark";
import { Button } from "../components/ui";
import { ApiError, api, fluxdUrl } from "../lib/api";
import { screenName } from "../lib/format";
import { useSession } from "../state/session";

let autoPairStarted = false;

function formatCode(raw: string): string {
  const clean = raw.toUpperCase().replace(/[^0-9A-Z]/g, "").slice(0, 8);
  return clean.length > 4 ? `${clean.slice(0, 4)}-${clean.slice(4)}` : clean;
}

/**
 * fluxd only takes orders from screens a person paired. The one-time code is
 * printed in the fluxd log (and secrets/pairing.code); it is never served by
 * the API, so a web page cannot pair itself.
 */
export function PairScreen() {
  const expired = useSession((s) => s.expired);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(expired ? "This screen's pairing ended (it was revoked or FluxOS was reset). Pair it again." : null);
  const input = useRef<HTMLInputElement>(null);

  async function pair(raw: string) {
    setBusy(true);
    setError(null);
    try {
      const r = await api.pair(raw, screenName());
      sessionStorage.setItem("fluxos.welcome", "1");
      useSession.getState().set({
        token: r.token,
        intentToken: r.intent_token,
        principal: r.principal,
        sessionId: r.session.id,
        name: r.session.name,
        mode: r.mode,
        agentUrl: r.agent_url,
        pairedAt: new Date().toISOString(),
      });
    } catch (e) {
      setBusy(false);
      setError(e instanceof ApiError ? e.message : "Pairing failed.");
      input.current?.focus();
    }
  }

  useEffect(() => {
    const m = /[#&]pair=([0-9A-Za-z-]{8,9})/.exec(window.location.hash);
    if (m?.[1] && !autoPairStarted) {
      autoPairStarted = true;
      window.history.replaceState(null, "", window.location.pathname + window.location.search);
      setCode(formatCode(m[1]));
      void pair(m[1]);
    } else {
      input.current?.focus();
    }
  }, []);

  const complete = code.replace("-", "").length === 8;

  return (
    <div className="fixed inset-0 overflow-hidden bg-bg">
      <img src={fluxdUrl("/v1/wallpaper.svg")} alt="" className="absolute inset-0 h-full w-full scale-105 object-cover opacity-70 blur-md" />
      <Aurora intensity={0.5} />
      <div className="absolute inset-0 bg-bg/40" />
      <div className="relative grid h-full place-items-center p-6">
        <form
          className="glass glow-ring fade-in w-[min(460px,100%)] rounded-xl p-8 shadow-raised"
          onSubmit={(e) => {
            e.preventDefault();
            if (complete) void pair(code);
          }}
        >
          <div className="flex items-center gap-3">
            <FluxMark size={40} />
            <div>
              <h1 className="font-display text-xl font-bold">Pair this screen</h1>
              <p className="text-sm text-muted">FluxOS only takes orders from screens you pair.</p>
            </div>
          </div>

          <label htmlFor="pair-code" className="mt-7 block text-sm font-medium">
            One-time code from the FluxOS console
          </label>
          <input
            id="pair-code"
            ref={input}
            value={code}
            onChange={(e) => setCode(formatCode(e.target.value))}
            placeholder="XXXX-XXXX"
            autoComplete="off"
            spellCheck={false}
            inputMode="text"
            aria-invalid={error ? true : undefined}
            className="mt-2 w-full rounded-lg border hairline bg-bg/60 px-4 py-3 text-center font-mono text-[28px] tracking-[0.3em] text-fg uppercase outline-none placeholder:text-fg/20 focus:border-accent"
          />
          {error && (
            <p role="alert" className="mt-3 text-sm text-bad">
              {error}
            </p>
          )}
          <Button type="submit" variant="primary" icon={KeyRound} busy={busy} disabled={!complete} className="mt-5 w-full">
            Pair screen
          </Button>

          <div className="mt-6 rounded-lg bg-fg/[0.05] p-4 text-sm text-muted">
            <p className="mb-2 flex items-center gap-2 font-medium text-fg">
              <ShieldCheck className="size-4 text-ok" /> Where is the code?
            </p>
            <ul className="space-y-1.5">
              <li>
                Development: <code className="font-mono text-fg">scripts/dev.sh</code> prints a link that pairs this screen, or run{" "}
                <code className="font-mono text-fg">scripts/flux pair</code>.
              </li>
              <li>
                NixOS: <code className="font-mono text-fg">sudo fluxd pair-code</code>
              </li>
            </ul>
            <p className="mt-2 text-xs">Codes work once. Five wrong tries rotate the code and pause pairing for 30 seconds.</p>
          </div>
        </form>
      </div>
    </div>
  );
}
