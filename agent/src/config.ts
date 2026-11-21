import { readFileSync } from "node:fs";

function env(name: string, fallback?: string): string | undefined {
  const v = process.env[name];
  return v !== undefined && v.trim() !== "" ? v.trim() : fallback;
}

/** Reads `NAME` or the file named by `NAME_FILE` (systemd credentials). */
function secret(name: string): string | undefined {
  const direct = env(name);
  if (direct) return direct;
  const file = env(`${name}_FILE`);
  if (!file) return undefined;
  try {
    return readFileSync(file, "utf8").trim();
  } catch {
    return undefined;
  }
}

export type Routing = "auto" | "local" | "llm";

export interface ProviderConfig {
  id: "anthropic" | "openai" | "ollama";
  model: string;
  baseUrl: string;
  apiKey?: string;
}

function providers(): ProviderConfig[] {
  const order = (env("FLUX_AI_PROVIDERS", "anthropic,openai,ollama") ?? "").split(",").map((s) => s.trim()).filter(Boolean);
  const all: Record<string, ProviderConfig | null> = {
    anthropic: secret("ANTHROPIC_API_KEY")
      ? {
          id: "anthropic",
          model: env("FLUX_ANTHROPIC_MODEL", "claude-sonnet-5-5")!,
          baseUrl: env("ANTHROPIC_BASE_URL", "https://api.anthropic.com")!,
          apiKey: secret("ANTHROPIC_API_KEY"),
        }
      : null,
    openai: secret("OPENAI_API_KEY")
      ? {
          id: "openai",
          model: env("FLUX_OPENAI_MODEL", "gpt-6.2-sol")!,
          baseUrl: env("OPENAI_BASE_URL", "https://api.openai.com/v1")!,
          apiKey: secret("OPENAI_API_KEY"),
        }
      : null,
    // Local models keep prompts and system data on the device.
    ollama:
      env("OLLAMA_HOST") || env("FLUX_OLLAMA_MODEL")
        ? {
            id: "ollama",
            model: env("FLUX_OLLAMA_MODEL", "qwen3:8b")!,
            baseUrl: env("OLLAMA_HOST", "http://127.0.0.1:11434")!.replace(/\/$/, ""),
          }
        : null,
  };
  return order.map((id) => all[id]).filter((p): p is ProviderConfig => !!p);
}

export const config = {
  host: env("FLUX_AGENT_HOST", "127.0.0.1")!,
  port: Number(env("FLUX_AGENT_PORT", "7701")),
  fluxdUrl: env("FLUXD_URL", "http://127.0.0.1:7700")!.replace(/\/$/, ""),
  fluxdToken: secret("FLUX_AGENT_TOKEN"),
  /** Clients must present this to submit intents. Unset means open on localhost (dev). */
  intentToken: secret("FLUX_INTENT_TOKEN"),
  providers: providers(),
  routing: (env("FLUX_AI_ROUTING", "auto") as Routing) ?? "auto",
  /** Minimum share of the request the local engine must understand to skip the LLM. */
  fastPathCoverage: Number(env("FLUX_FAST_PATH_COVERAGE", "0.6")),
  maxRepairs: Number(env("FLUX_MAX_REPAIRS", "2")),
  llmTimeoutMs: Number(env("FLUX_LLM_TIMEOUT_MS", "60000")),
  // The web UI: Vite dev server (5173) or served by fluxd (7700).
  corsOrigins: (env("FLUX_CORS_ORIGINS", "http://localhost:5173,http://127.0.0.1:5173,http://127.0.0.1:7700,http://localhost:7700") ?? "")
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean),
};

export type Config = typeof config;
