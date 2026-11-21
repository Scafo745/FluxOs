import type { ProviderConfig } from "../config.js";
import { type CompletionRequest, type CompletionResult, type Provider, ProviderError, postJson } from "./types.js";

/** Local models through Ollama: prompts and system data never leave the device. */
export class OllamaProvider implements Provider {
  readonly id = "ollama";
  readonly local = true;
  readonly model: string;

  constructor(private readonly cfg: ProviderConfig) {
    this.model = cfg.model;
  }

  async complete(req: CompletionRequest, signal: AbortSignal): Promise<CompletionResult> {
    const started = Date.now();
    const data = await postJson(
      `${this.cfg.baseUrl}/api/chat`,
      {},
      {
        model: this.model,
        stream: false,
        // Structured outputs: constrain decoding to the plan schema.
        format: req.schema,
        options: { temperature: 0.2, num_predict: req.maxTokens },
        messages: [{ role: "system", content: req.system }, ...req.messages],
      },
      signal,
      this.id,
    );
    const text: string | undefined = data.message?.content;
    if (!text) throw new ProviderError(this.id, "empty response", true);
    return {
      text,
      provider: this.id,
      model: data.model ?? this.model,
      ms: Date.now() - started,
      usage: { input: data.prompt_eval_count, output: data.eval_count },
    };
  }
}
