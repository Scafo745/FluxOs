import type { ProviderConfig } from "../config.js";
import { type CompletionRequest, type CompletionResult, type Provider, ProviderError, postJson } from "./types.js";

/**
 * OpenAI Chat Completions, or any compatible endpoint (OpenRouter, Groq,
 * vLLM, LM Studio) via OPENAI_BASE_URL. Uses JSON mode, which compatible
 * servers support more widely than strict schemas.
 */
export class OpenAIProvider implements Provider {
  readonly id = "openai";
  readonly local = false;
  readonly model: string;

  constructor(private readonly cfg: ProviderConfig) {
    this.model = cfg.model;
  }

  async complete(req: CompletionRequest, signal: AbortSignal): Promise<CompletionResult> {
    const started = Date.now();
    const data = await postJson(
      `${this.cfg.baseUrl.replace(/\/$/, "")}/chat/completions`,
      { authorization: `Bearer ${this.cfg.apiKey ?? ""}` },
      {
        model: this.model,
        messages: [{ role: "system", content: req.system }, ...req.messages],
        response_format: { type: "json_object" },
      },
      signal,
      this.id,
    );
    const text: string | undefined = data.choices?.[0]?.message?.content;
    if (!text) throw new ProviderError(this.id, "empty response", true);
    return {
      text,
      provider: this.id,
      model: data.model ?? this.model,
      ms: Date.now() - started,
      usage: { input: data.usage?.prompt_tokens, output: data.usage?.completion_tokens },
    };
  }
}
