// Retrieval over industry knowledge, profiles, the package catalog and the
// tool registry. BM25 keeps it dependency-free and fully local; the Doc
// interface is the seam for a vector database later.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

export interface Doc {
  id: string;
  kind: "knowledge" | "profile" | "package" | "tool";
  title: string;
  text: string;
}

const STOP = new Set(
  "a an and are as at be but by can do does for from get give how i if in into is it its just let like make me my need no not of on or our please put set so some than that the their them then there these this to too up us use want was we what when where which while who why will with would you your".split(
    " ",
  ),
);

function stem(w: string): string {
  if (w.length > 5 && w.endsWith("ing")) return w.slice(0, -3);
  if (w.length > 4 && w.endsWith("ed")) return w.slice(0, -2);
  if (w.length > 3 && w.endsWith("s") && !w.endsWith("ss")) return w.slice(0, -1);
  return w;
}

export function tokenize(s: string): string[] {
  return (s.toLowerCase().match(/[a-z0-9][a-z0-9_+-]*/g) ?? []).filter((w) => !STOP.has(w)).map(stem);
}

export class Bm25Index {
  private readonly tf: Map<string, number>[] = [];
  private readonly df = new Map<string, number>();
  private readonly len: number[] = [];
  private avg = 0;

  constructor(readonly docs: Doc[]) {
    for (const d of docs) {
      const toks = tokenize(`${d.title} ${d.title} ${d.text}`);
      const tf = new Map<string, number>();
      for (const t of toks) tf.set(t, (tf.get(t) ?? 0) + 1);
      for (const t of tf.keys()) this.df.set(t, (this.df.get(t) ?? 0) + 1);
      this.tf.push(tf);
      this.len.push(toks.length);
    }
    this.avg = this.len.reduce((a, b) => a + b, 0) / Math.max(1, this.len.length);
  }

  search(query: string, k = 6, kinds?: Doc["kind"][]): { doc: Doc; score: number }[] {
    const q = [...new Set(tokenize(query))];
    const n = this.docs.length;
    const k1 = 1.2;
    const b = 0.75;
    const scored: { doc: Doc; score: number }[] = [];
    this.docs.forEach((doc, i) => {
      if (kinds && !kinds.includes(doc.kind)) return;
      let score = 0;
      for (const t of q) {
        const f = this.tf[i]!.get(t);
        if (!f) continue;
        const df = this.df.get(t) ?? 0;
        const idf = Math.log(1 + (n - df + 0.5) / (df + 0.5));
        score += (idf * f * (k1 + 1)) / (f + k1 * (1 - b + (b * this.len[i]!) / this.avg));
      }
      if (score > 0) scored.push({ doc, score });
    });
    return scored.sort((a, b2) => b2.score - a.score).slice(0, k);
  }
}

export function knowledgeDir(): string {
  return process.env.FLUX_KNOWLEDGE_DIR ?? fileURLToPath(new URL("../knowledge/", import.meta.url));
}

/** Splits markdown files into one doc per `##` section. */
export function loadKnowledge(dir = knowledgeDir()): Doc[] {
  const docs: Doc[] = [];
  let files: string[] = [];
  try {
    files = readdirSync(dir).filter((f) => f.endsWith(".md"));
  } catch {
    return docs;
  }
  for (const f of files) {
    const src = readFileSync(join(dir, f), "utf8");
    const title = src.match(/^#\s+(.+)$/m)?.[1]?.trim() ?? f;
    const sections = src.split(/^##\s+/m);
    sections.slice(1).forEach((sec, i) => {
      const [head, ...body] = sec.split("\n");
      docs.push({ id: `${f}#${i}`, kind: "knowledge", title: `${title}: ${head?.trim()}`, text: body.join("\n").trim() });
    });
  }
  return docs;
}
