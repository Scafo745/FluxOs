import type { LucideIcon } from "lucide-react";
import {
  ArrowLeft,
  ArrowUp,
  Eye,
  EyeOff,
  File,
  FileCode,
  FileImage,
  FileText,
  Folder,
  FolderCog,
  HardDrive,
  House,
  Link2,
  Palette,
  RefreshCw,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { CodeView } from "../components/CodeView";
import { Empty, IconButton, Spinner } from "../components/ui";
import { api } from "../lib/api";
import { ago, bytes } from "../lib/format";
import type { DirListing, FileContent, FileEntry } from "../lib/types";
import { errorText } from "../state/actions";
import type { Win } from "../state/shell";

const PLACES: { label: string; path: string; icon: LucideIcon; hint?: string }[] = [
  { label: "Home", path: "~", icon: House },
  { label: "Documents", path: "~/Documents", icon: Folder },
  { label: "Downloads", path: "~/Downloads", icon: Folder },
  { label: "Pictures", path: "~/Pictures", icon: Folder },
  { label: "Live theme", path: "/etc/fluxos", icon: Palette, hint: "Files fluxd swaps for every generation" },
  { label: "System config", path: "/etc/nixos", icon: FolderCog, hint: "The FluxOS flake that nixos-rebuild uses" },
  { label: "Computer", path: "/", icon: HardDrive },
];

function iconFor(e: FileEntry): LucideIcon {
  if (e.kind === "dir") return Folder;
  if (e.kind === "link") return Link2;
  const ext = e.name.split(".").pop()?.toLowerCase() ?? "";
  if (["png", "jpg", "jpeg", "gif", "webp", "svg", "ico", "bmp"].includes(ext)) return FileImage;
  if (["nix", "rs", "ts", "tsx", "js", "json", "toml", "sh", "py", "css", "html", "yml", "yaml"].includes(ext)) return FileCode;
  if (["md", "txt", "log", "conf", "ini", "cfg"].includes(ext)) return FileText;
  return File;
}

function join(dir: string, name: string): string {
  return dir === "/" ? `/${name}` : `${dir}/${name}`;
}

/** The console user's files, with that user's permissions (read only for now). */
export function Files({ win }: { win: Win }) {
  const [path, setPath] = useState(String(win.props.path ?? "~"));
  const [history, setHistory] = useState<string[]>([]);
  const [listing, setListing] = useState<DirListing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [hidden, setHidden] = useState(false);
  const [selected, setSelected] = useState<FileEntry | null>(null);
  const [preview, setPreview] = useState<FileContent | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);

  const load = useCallback((p: string) => {
    setBusy(true);
    api
      .files(p)
      .then((l) => {
        setListing(l);
        setError(null);
      })
      .catch((e) => setError(errorText(e)))
      .finally(() => setBusy(false));
  }, []);

  useEffect(() => {
    setSelected(null);
    setPreview(null);
    load(path);
  }, [path, load]);

  const go = (p: string) => {
    setHistory((h) => [...h.slice(-30), listing?.path ?? path]);
    setPath(p);
  };

  const open = (e: FileEntry) => {
    if (!listing) return;
    const full = join(listing.path, e.name);
    if (e.kind === "dir") {
      go(full);
      return;
    }
    setSelected(e);
    setPreview(null);
    setPreviewError(null);
    api
      .file(full)
      .then(setPreview)
      .catch((err) => setPreviewError(errorText(err)));
  };

  const entries = useMemo(() => (listing?.entries ?? []).filter((e) => hidden || !e.hidden), [listing, hidden]);
  const crumbs = useMemo(() => {
    const p = listing?.path ?? "/";
    const parts = p.split("/").filter(Boolean);
    return [{ label: "/", path: "/" }, ...parts.map((part, i) => ({ label: part, path: `/${parts.slice(0, i + 1).join("/")}` }))];
  }, [listing]);

  return (
    <div className="flex h-full">
      <aside className="w-48 shrink-0 space-y-0.5 overflow-y-auto border-r hairline p-2 text-sm">
        <p className="px-2 pt-1 pb-1.5 text-xs font-semibold tracking-[0.12em] text-muted uppercase">Places</p>
        {PLACES.map((pl) => {
          const Icon = pl.icon;
          const active = listing && (pl.path === "~" ? listing.path === listing.home : listing.path === pl.path.replace(/^~/, listing.home));
          return (
            <button
              key={pl.path}
              type="button"
              title={pl.hint}
              onClick={() => go(pl.path)}
              className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left ${active ? "bg-accent/15 text-fg" : "text-fg/85 hover:bg-fg/[0.06]"}`}
            >
              <Icon className="size-4 shrink-0 text-accent" /> {pl.label}
            </button>
          );
        })}
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex items-center gap-1 border-b hairline px-2 py-1.5">
          <IconButton
            icon={ArrowLeft}
            label="Back"
            className="size-7"
            disabled={!history.length}
            onClick={() => {
              setPath(history[history.length - 1] ?? "~");
              setHistory((h) => h.slice(0, -1));
            }}
          />
          <IconButton icon={ArrowUp} label="Up" className="size-7" disabled={!listing?.parent} onClick={() => listing?.parent && go(listing.parent)} />
          <nav className="flex min-w-0 flex-1 items-center overflow-x-auto text-sm" aria-label="Path">
            {crumbs.map((c, i) => (
              <span key={c.path} className="flex shrink-0 items-center">
                {i > 1 && <span className="px-0.5 text-muted">/</span>}
                <button type="button" onClick={() => go(c.path)} className="rounded px-1.5 py-0.5 hover:bg-fg/[0.07]">
                  {c.label}
                </button>
              </span>
            ))}
          </nav>
          {busy && <Spinner className="size-3.5 text-muted" />}
          <IconButton icon={hidden ? EyeOff : Eye} label={hidden ? "Hide hidden files" : "Show hidden files"} className="size-7" onClick={() => setHidden((v) => !v)} />
          <IconButton icon={RefreshCw} label="Reload" className="size-7" onClick={() => load(path)} />
        </div>

        <div className="flex min-h-0 flex-1">
          <div className="min-w-0 flex-1 overflow-y-auto">
            {error ? (
              <p className="p-5 text-sm text-bad">{error}</p>
            ) : entries.length === 0 && listing ? (
              <p className="p-5 text-sm text-muted">This folder is empty.</p>
            ) : (
              <table className="w-full text-sm">
                <thead className="sticky top-0 bg-surface text-xs text-muted">
                  <tr className="text-left">
                    <th className="px-3 py-1.5 font-medium">Name</th>
                    <th className="w-24 px-3 py-1.5 text-right font-medium">Size</th>
                    <th className="w-28 px-3 py-1.5 font-medium">Modified</th>
                  </tr>
                </thead>
                <tbody>
                  {entries.map((e) => {
                    const Icon = iconFor(e);
                    return (
                      <tr
                        key={e.name}
                        onClick={() => (e.kind === "dir" ? undefined : open(e))}
                        onDoubleClick={() => open(e)}
                        className={`cursor-default ${selected?.name === e.name ? "bg-accent/15" : "hover:bg-fg/[0.04]"}`}
                      >
                        <td className="px-3 py-1">
                          <span className={`flex items-center gap-2 ${e.hidden ? "text-muted" : ""}`}>
                            <Icon className={`size-4 shrink-0 ${e.kind === "dir" ? "text-accent" : "text-muted"}`} />
                            <span className="truncate">{e.name}</span>
                          </span>
                        </td>
                        <td className="px-3 py-1 text-right text-muted tabular-nums">{e.kind === "dir" ? "" : bytes(e.size)}</td>
                        <td className="px-3 py-1 text-muted">{e.modified ? ago(e.modified * 1000) : ""}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
            {listing?.truncated && <p className="p-3 text-xs text-muted">Showing the first 5000 entries.</p>}
          </div>

          {selected && (
            <section className="flex w-[46%] min-w-72 flex-col border-l hairline">
              <div className="border-b hairline px-3 py-2">
                <p className="truncate font-semibold">{selected.name}</p>
                <p className="text-xs text-muted">
                  {bytes(selected.size)} · mode {selected.mode}
                  {preview?.truncated ? " · first 512 KB" : ""}
                </p>
              </div>
              <div className="min-h-0 flex-1 overflow-auto">
                {previewError ? (
                  <p className="p-4 text-sm text-bad">{previewError}</p>
                ) : !preview ? (
                  <p className="flex items-center gap-2 p-4 text-sm text-muted">
                    <Spinner /> Loading…
                  </p>
                ) : preview.data_url ? (
                  <div className="grid h-full place-items-center p-4">
                    <img src={preview.data_url} alt={selected.name} className="max-h-full max-w-full rounded-md shadow-raised" />
                  </div>
                ) : preview.binary ? (
                  <Empty icon={File} title="Binary file">
                    This file has no text preview.
                  </Empty>
                ) : (
                  <CodeView text={preview.text ?? ""} path={selected.name} />
                )}
              </div>
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
