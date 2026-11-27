import { useMemo } from "react";

import { TOKEN_CLASS, highlight, langOf } from "../lib/highlight";
import { Spinner } from "./ui";

/** Read-only view of a generated file, highlighted, with line numbers. */
export function CodeView({ text, path, error }: { text: string | null; path: string; error?: string | null }) {
  const lines = useMemo(() => (text === null ? [] : highlight(text, langOf(path))), [text, path]);
  if (error) return <p className="p-5 text-sm text-bad">{error}</p>;
  if (text === null)
    return (
      <div className="flex items-center gap-2 p-5 text-sm text-muted">
        <Spinner /> Loading {path}
      </div>
    );
  const width = String(lines.length).length;
  return (
    <div className="code-scroll min-w-max py-3">
      {lines.map((toks, i) => (
        <div key={i} className="flex hover:bg-fg/[0.04]">
          <span className="sticky left-0 shrink-0 bg-surface pr-4 pl-4 text-right text-muted/50 select-none" style={{ width: `${width + 3}ch` }}>
            {i + 1}
          </span>
          <span className="pr-6 whitespace-pre">
            {toks.length === 0
              ? " "
              : toks.map((t, j) => (
                  <span key={j} className={t.c ? TOKEN_CLASS[t.c] : undefined}>
                    {t.t}
                  </span>
                ))}
          </span>
        </div>
      ))}
    </div>
  );
}
