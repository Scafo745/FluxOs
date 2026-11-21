type Level = "debug" | "info" | "warn" | "error";

const order: Record<Level, number> = { debug: 10, info: 20, warn: 30, error: 40 };
const min = order[(process.env.FLUX_LOG_LEVEL as Level) ?? "info"] ?? 20;

function emit(level: Level, msg: string, fields?: Record<string, unknown>): void {
  if (order[level] < min) return;
  const line = JSON.stringify({ ts: new Date().toISOString(), level, target: "flux-agent", msg, ...fields });
  (level === "error" || level === "warn" ? process.stderr : process.stdout).write(line + "\n");
}

export const log = {
  debug: (m: string, f?: Record<string, unknown>) => emit("debug", m, f),
  info: (m: string, f?: Record<string, unknown>) => emit("info", m, f),
  warn: (m: string, f?: Record<string, unknown>) => emit("warn", m, f),
  error: (m: string, f?: Record<string, unknown>) => emit("error", m, f),
};
