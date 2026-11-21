export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export interface CompletionRequest {
  system: string;
  messages: ChatMessage[];
  /** JSON Schema of the expected object (used where the provider supports it). */
  schema: Record<string, unknown>;
  maxTokens: number;
}

export interface CompletionResult {
  text: string;
  provider: string;
  model: string;
  ms: number;
  usage?: { input?: number; output?: number };
}

export interface Provider {
  readonly id: string;
  readonly model: string;
  /** Local providers keep prompts on the device. */
  readonly local: boolean;
  complete(req: CompletionRequest, signal: AbortSignal): Promise<CompletionResult>;
}

export class ProviderError extends Error {
  constructor(
    readonly provider: string,
    message: string,
    readonly retryable: boolean,
  ) {
    super(`${provider}: ${message}`);
  }
}

export async function postJson(url: string, headers: Record<string, string>, body: unknown, signal: AbortSignal, provider: string): Promise<any> {
  let res: Response;
  try {
    res = await fetch(url, { method: "POST", headers: { "content-type": "application/json", ...headers }, body: JSON.stringify(body), signal });
  } catch (e) {
    throw new ProviderError(provider, `request failed: ${(e as Error).message}`, true);
  }
  const text = await res.text();
  if (!res.ok) {
    // 429 and 5xx are worth failing over; 4xx usually means bad config.
    const retryable = res.status === 429 || res.status >= 500;
    throw new ProviderError(provider, `HTTP ${res.status}: ${text.slice(0, 300)}`, retryable);
  }
  try {
    return JSON.parse(text);
  } catch {
    throw new ProviderError(provider, "response was not JSON", true);
  }
}
