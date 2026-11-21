// Small syntax highlighter for the files fluxd renders (Nix, JSON, CSS,
// INI-style configs). It tokenizes the whole text, so multi-line strings and
// comments are handled, then splits tokens into lines.

export type Lang = "nix" | "json" | "css" | "ini" | "js" | "text";
export interface Tok {
  t: string;
  c?: "kw" | "str" | "num" | "com" | "attr" | "punct" | "const";
}

export function langOf(path: string): Lang {
  if (path.endsWith(".nix")) return "nix";
  if (path.endsWith(".json")) return "json";
  if (path.endsWith(".css")) return "css";
  if (path.endsWith(".js")) return "js";
  if (path.endsWith(".svg")) return "text";
  return "ini";
}

const NIX_KW = new Set(["let", "in", "with", "rec", "inherit", "if", "then", "else", "assert", "import", "or"]);
const JS_KW = new Set(["var", "let", "const", "for", "if", "else", "new", "function", "return", "true", "false"]);

function nixIndented(src: string, i: number): number {
  // i points just after the opening ''
  let j = i;
  while (j < src.length) {
    if (src.startsWith("''", j)) {
      const next = src[j + 2];
      if (next === "'" || next === "$") {
        j += 3;
        continue;
      }
      if (next === "\\") {
        j += 4;
        continue;
      }
      return j + 2;
    }
    j++;
  }
  return src.length;
}

function quoted(src: string, i: number, q: string): number {
  let j = i + 1;
  while (j < src.length && src[j] !== q) {
    if (src[j] === "\\") j++;
    if (src[j] === "\n" && q !== '"') break;
    j++;
  }
  return Math.min(j + 1, src.length);
}

function tokenize(src: string, lang: Lang): Tok[] {
  const out: Tok[] = [];
  const push = (t: string, c?: Tok["c"]) => {
    if (t) out.push({ t, c });
  };
  if (lang === "text") return [{ t: src }];
  let i = 0;
  let plain = "";
  const flush = () => {
    push(plain);
    plain = "";
  };
  const atLineStart = () => i === 0 || src[i - 1] === "\n";
  while (i < src.length) {
    const ch = src[i] ?? "";
    const rest = src.slice(i, i + 3);
    // comments
    if ((lang === "nix" && ch === "#") || (lang === "ini" && (ch === "#" || ch === ";") && atLineStart()) || ((lang === "js") && rest.startsWith("//"))) {
      flush();
      const end = src.indexOf("\n", i);
      const j = end < 0 ? src.length : end;
      push(src.slice(i, j), "com");
      i = j;
      continue;
    }
    if ((lang === "nix" || lang === "css" || lang === "js") && rest.startsWith("/*")) {
      flush();
      const end = src.indexOf("*/", i + 2);
      const j = end < 0 ? src.length : end + 2;
      push(src.slice(i, j), "com");
      i = j;
      continue;
    }
    // strings
    if (lang === "nix" && rest.startsWith("''")) {
      flush();
      const j = nixIndented(src, i + 2);
      push(src.slice(i, j), "str");
      i = j;
      continue;
    }
    if (ch === '"' || ((lang === "css" || lang === "js") && ch === "'")) {
      flush();
      const j = quoted(src, i, ch);
      const s = src.slice(i, j);
      const key = lang === "json" && /^\s*:/.test(src.slice(j, j + 4));
      push(s, key ? "attr" : "str");
      i = j;
      continue;
    }
    // ini section headers and keys
    if (lang === "ini" && atLineStart() && ch === "[") {
      flush();
      const end = src.indexOf("\n", i);
      const j = end < 0 ? src.length : end;
      push(src.slice(i, j), "kw");
      i = j;
      continue;
    }
    if (lang === "ini" && atLineStart()) {
      const m = /^[A-Za-z0-9_.\-]+(?=\s*=)/.exec(src.slice(i));
      if (m) {
        flush();
        push(m[0], "attr");
        i += m[0].length;
        continue;
      }
    }
    // css custom properties and hex colors
    if (lang === "css") {
      const m = /^--[A-Za-z0-9-]+/.exec(src.slice(i, i + 64));
      if (m) {
        flush();
        push(m[0], "attr");
        i += m[0].length;
        continue;
      }
      const h = /^#[0-9a-fA-F]{3,8}\b/.exec(src.slice(i, i + 10));
      if (h) {
        flush();
        push(h[0], "num");
        i += h[0].length;
        continue;
      }
    }
    // numbers
    if (/[0-9]/.test(ch) && !/[A-Za-z0-9_]/.test(src[i - 1] ?? "")) {
      const m = /^[0-9]+(\.[0-9]+)?(px|ms|s|%|rem|em)?/.exec(src.slice(i, i + 24));
      if (m) {
        flush();
        push(m[0], "num");
        i += m[0].length;
        continue;
      }
    }
    // identifiers
    if (/[A-Za-z_]/.test(ch) && !/[A-Za-z0-9_'-]/.test(src[i - 1] ?? "")) {
      const m = /^[A-Za-z_][A-Za-z0-9_'-]*/.exec(src.slice(i, i + 80));
      if (m) {
        const word = m[0];
        const after = src.slice(i + word.length, i + word.length + 3);
        flush();
        if (word === "true" || word === "false" || word === "null") push(word, "const");
        else if (lang === "nix" && NIX_KW.has(word)) push(word, "kw");
        else if (lang === "js" && JS_KW.has(word)) push(word, "kw");
        else if (lang === "nix" && /^\s*=(?!=)/.test(after)) push(word, "attr");
        else push(word);
        i += word.length;
        continue;
      }
    }
    if ("{}[]();=:,.".includes(ch)) {
      flush();
      push(ch, "punct");
      i++;
      continue;
    }
    plain += ch;
    i++;
  }
  flush();
  return out;
}

/** Tokens split into lines. */
export function highlight(src: string, lang: Lang): Tok[][] {
  const lines: Tok[][] = [[]];
  for (const tok of tokenize(src, lang)) {
    const parts = tok.t.split("\n");
    parts.forEach((p, idx) => {
      if (idx > 0) lines.push([]);
      if (p) lines[lines.length - 1]?.push({ t: p, c: tok.c });
    });
  }
  return lines;
}

export const TOKEN_CLASS: Record<NonNullable<Tok["c"]>, string> = {
  kw: "text-secondary",
  str: "text-ok",
  num: "text-warn",
  com: "text-muted italic",
  attr: "text-accent",
  punct: "text-fg/55",
  const: "text-secondary",
};
