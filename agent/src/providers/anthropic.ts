import type { ProviderConfig } from "../config.js";
import { type CompletionRequest, type CompletionResult, type Provider, ProviderError, postJson } from "./types.js";

/** Claude via the Messages API. The plan comes back as a JSON text block. */
export class AnthropicProvider implements Provider {
  readonly id = "anthropic";
  readonly local = false;
  readonly model: string;

  constructor(private readonly cfg: ProviderConfig) {
    this.model = cfg.model;
  }

  async complete(req: CompletionRequest, signal: AbortSignal): Promise<CompletionResult> {
    const started = Date.now();
    const data = await postJson(
      `${this.cfg.baseUrl.replace(/\/$/, "")}/v1/messages`,
      { "x-api-key": this.cfg.apiKey ?? "", "anthropic-version": "2023-06-01" },
      {
        model: this.model,
        max_tokens: req.maxTokens,
        system: req.system,
        messages: req.messages.map((m) => ({ role: m.role, content: m.content })),
      },
      signal,
      this.id,
    );
    const text = (data.content ?? [])
      .filter((b: { type: string }) => b.type === "text")
      .map((b: { text: string }) => b.text)
      .join("");
    if (!text) throw new ProviderError(this.id, `empty response (stop_reason ${data.stop_reason})`, true);
    return {
      text,
      provider: this.id,
      model: data.model ?? this.model,
      ms: Date.now() - started,
      usage: { input: data.usage?.input_tokens, output: data.usage?.output_tokens },
    };
  }
}
