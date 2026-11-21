import { Eye, FileCode, GitCommitVertical, Power, Undo2 } from "lucide-react";
import { useState } from "react";

import { ArrowText, Button, Chip, Empty, TierBadge } from "../components/ui";
import { GEN_STATUS, ago } from "../lib/format";
import type { GenerationMeta } from "../lib/types";
import { errorText, restoreGeneration } from "../state/actions";
import { useShell } from "../state/shell";
import { useSystem } from "../state/system";

function Row({ g, current, last }: { g: GenerationMeta; current: boolean; last: boolean }) {
  const [busy, setBusy] = useState<"restore" | "preview" | null>(null);
  const [open, setOpen] = useState(false);
  const st = GEN_STATUS[g.status];
  const restorable = !current && (g.status === "active" || g.status === "superseded" || g.status === "staged");
  const ring = current ? "border-accent bg-accent/15 text-accent" : g.status === "failed" || g.status === "rolled-back" ? "border-bad/60 text-bad" : "hairline text-fg/80";
  return (
    <li className="relative flex gap-4 pb-4">
      {!last && <span className="absolute top-11 bottom-0 left-[19px] w-px bg-line/70" />}
      <div className={`grid size-10 shrink-0 place-items-center rounded-full border-2 bg-surface font-mono text-sm font-bold ${ring}`}>{g.number}</div>
      <div className={`min-w-0 flex-1 rounded-lg border p-3 ${current ? "border-accent/40 bg-accent/[0.06]" : "hairline bg-fg/[0.03]"}`}>
        <div className="flex flex-wrap items-center gap-2">
          <span className="font-semibold">{g.theme_name}</span>
          {current && <Chip className="border-accent/40 text-accent">Current</Chip>}
          <span className={`text-xs font-semibold ${st.tone}`}>{st.label}</span>
          {g.requires_reboot && (
            <span className="flex items-center gap-1 text-xs text-warn" title="Needed a restart">
              <Power className="size-3" /> restart
            </span>
          )}
          <div className="flex-1" />
          <TierBadge tier={g.tier} label={false} />
        </div>
        <button type="button" aria-expanded={open} onClick={() => setOpen((v) => !v)} className="mt-1 block w-full text-left text-sm text-muted">
          <span className={open ? "" : "line-clamp-2"}>
            <ArrowText text={g.summary} />
          </span>
        </button>
        <p className="mt-1 text-xs text-muted">
          {g.actor} · {ago(g.created_at)}
        </p>
        <div className="mt-2.5 flex flex-wrap gap-2">
          {restorable && (
            <Button
              size="sm"
              icon={Undo2}
              busy={busy === "restore"}
              onClick={async () => {
                setBusy("restore");
                await restoreGeneration(g.number);
                setBusy(null);
              }}
            >
              Restore
            </Button>
          )}
          {restorable && (
            <Button
              size="sm"
              variant="ghost"
              icon={Eye}
              busy={busy === "preview"}
              onClick={async () => {
                setBusy("preview");
                try {
                  await useSystem.getState().previewGeneration(g.number);
                } catch (e) {
                  useShell.getState().toast({ tone: "danger", title: "Preview failed", body: errorText(e) });
                } finally {
                  setBusy(null);
                }
              }}
            >
              Preview
            </Button>
          )}
          <Button
            size="sm"
            variant="ghost"
            icon={FileCode}
            onClick={() => useShell.getState().open("viewer", { generation: g.number, path: "etc/nixos/fluxos/generated.nix" }, `generated.nix · generation ${g.number}`)}
          >
            NixOS config
          </Button>
        </div>
      </div>
    </li>
  );
}

export function Timeline() {
  const gens = useSystem((s) => s.generations);
  const current = useSystem((s) => s.system?.generation);
  if (!gens.length) return <Empty icon={GitCommitVertical} title="No generations yet" />;
  return (
    <div className="p-5">
      <p className="mb-5 text-sm text-muted">
        Every change builds a new generation next to the old ones. Restoring one is itself a validated change, so FluxOS asks first when going back touches the system.
      </p>
      <ol>
        {gens.map((g, i) => (
          <Row key={g.number} g={g} current={g.number === current} last={i === gens.length - 1} />
        ))}
      </ol>
    </div>
  );
}
