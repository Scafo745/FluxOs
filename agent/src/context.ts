// Loads what the planner needs from fluxd. Registry, catalog and profiles
// change rarely and are cached; system state is fetched per request.

import { Fluxd } from "./fluxd.js";
import { Bm25Index, type Doc } from "./retrieval.js";
import type { Catalog, Hardware, ProfileSummary, SystemView, ToolInfo } from "./types.js";

export interface Context {
  system: SystemView & { spec_digest?: string };
  hardware: Hardware;
  tools: ToolInfo[];
  catalog: Catalog;
  profiles: ProfileSummary[];
  index: Bm25Index;
}

const STATIC_TTL_MS = 5 * 60_000;

export class ContextLoader {
  private cached?: { at: number; tools: ToolInfo[]; catalog: Catalog; profiles: ProfileSummary[]; hardware: Hardware; index: Bm25Index };

  constructor(
    private readonly fluxd: Fluxd,
    private readonly knowledge: Doc[],
  ) {}

  invalidate() {
    this.cached = undefined;
  }

  private async loadStatic() {
    if (this.cached && Date.now() - this.cached.at < STATIC_TTL_MS) return this.cached;
    const [tools, catalog, profiles, hardware] = await Promise.all([this.fluxd.tools(), this.fluxd.catalog(), this.fluxd.profiles(), this.fluxd.hardware()]);
    const docs: Doc[] = [
      ...this.knowledge,
      ...profiles.map((p) => ({
        id: `profile:${p.id}`,
        kind: "profile" as const,
        title: `Profile ${p.id}: ${p.name}`,
        text: `${p.summary} Tags: ${p.tags.join(", ")}. ${p.status === "preview" ? "Preview profile; compliance review required." : ""}`,
      })),
      ...catalog.packages.map((p) => ({
        id: `package:${p.id}`,
        kind: "package" as const,
        title: `${p.name} (${p.id})`,
        text: `${p.description}. Category ${p.category}. Tags: ${p.tags.join(", ")}. ${p.flatpak ? "Available from Flathub." : "System package."}`,
      })),
      ...tools
        .filter((t) => !t.blocked)
        .map((t) => ({ id: `tool:${t.name}`, kind: "tool" as const, title: `Tool ${t.name}`, text: `${t.title}. ${t.description}` })),
    ];
    this.cached = { at: Date.now(), tools, catalog, profiles, hardware, index: new Bm25Index(docs) };
    return this.cached;
  }

  async load(): Promise<Context> {
    const [s, system] = await Promise.all([this.loadStatic(), this.fluxd.system()]);
    return { system, hardware: s.hardware, tools: s.tools, catalog: s.catalog, profiles: s.profiles, index: s.index };
  }
}
