// Server-sent events over fetch, so requests can carry an Authorization
// header (EventSource cannot) and POST bodies (the agent's intent stream).

export interface SseMessage {
  event: string;
  data: string;
  id?: string;
}

/** Reads an SSE response body until it ends or the signal aborts. */
export async function readSse(res: Response, onMessage: (m: SseMessage) => void): Promise<void> {
  if (!res.body) throw new Error("response has no body");
  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = "";
  let event = "message";
  let data: string[] = [];
  let id: string | undefined;

  const line = (l: string) => {
    if (l === "") {
      if (data.length) onMessage({ event, data: data.join("\n"), id });
      event = "message";
      data = [];
      id = undefined;
      return;
    }
    if (l.startsWith(":")) return;
    const colon = l.indexOf(":");
    const field = colon < 0 ? l : l.slice(0, colon);
    let value = colon < 0 ? "" : l.slice(colon + 1);
    if (value.startsWith(" ")) value = value.slice(1);
    if (field === "event") event = value;
    else if (field === "data") data.push(value);
    else if (field === "id") id = value;
  };

  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });
    let nl = buf.indexOf("\n");
    while (nl >= 0) {
      line(buf.slice(0, nl).replace(/\r$/, ""));
      buf = buf.slice(nl + 1);
      nl = buf.indexOf("\n");
    }
  }
  if (buf) line(buf);
  line("");
}

export interface Subscription {
  close(): void;
}

/**
 * Keeps a GET event stream open, reconnecting with backoff and resuming
 * from the last sequence number seen.
 */
export function subscribe(opts: {
  url: (since: number) => string;
  headers: () => Record<string, string>;
  since: number;
  onMessage: (m: SseMessage) => void;
  onStatus: (online: boolean) => void;
  onAuthError: () => void;
}): Subscription {
  let since = opts.since;
  let closed = false;
  let controller: AbortController | null = null;
  let delay = 500;

  const run = async () => {
    while (!closed) {
      controller = new AbortController();
      try {
        const res = await fetch(opts.url(since), { headers: opts.headers(), signal: controller.signal });
        if (res.status === 401) {
          opts.onAuthError();
          return;
        }
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        opts.onStatus(true);
        delay = 500;
        await readSse(res, (m) => {
          if (m.id) since = Number(m.id) || since;
          opts.onMessage(m);
        });
      } catch {
        if (closed) return;
      }
      if (closed) return;
      opts.onStatus(false);
      await new Promise((r) => setTimeout(r, delay));
      delay = Math.min(delay * 2, 5000);
    }
  };
  void run();

  return {
    close() {
      closed = true;
      controller?.abort();
    },
  };
}
