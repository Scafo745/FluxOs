// Scripted LLM endpoints for scripts/verify/agent.sh. Not a real model.
//   MOCK_FAIL_PORT (17798): Anthropic-style endpoint that always answers 529, to test failover
//   MOCK_OK_PORT   (17799): OpenAI-compatible chat completions with canned plans
import http from "node:http";

const failPort = Number(process.env.MOCK_FAIL_PORT ?? 17798);
const okPort = Number(process.env.MOCK_OK_PORT ?? 17799);

function read(req) {
  return new Promise((resolve) => {
    let body = "";
    req.on("data", (c) => (body += c));
    req.on("end", () => resolve(body));
  });
}

http
  .createServer(async (req, res) => {
    await read(req);
    res.writeHead(529, { "content-type": "application/json" });
    res.end(JSON.stringify({ type: "error", error: { type: "overloaded_error", message: "Overloaded" } }));
  })
  .listen(failPort, "127.0.0.1");

function planFor(firstMessage, last) {
  // The agent prepends conversation history; react only to the current request.
  const first = firstMessage.match(/^Request: (.*)$/m)?.[1] ?? firstMessage;
  if (last.includes("FluxOS rejected that plan")) {
    // Repair turn: fix the tool name and the font the validator rejected.
    return {
      kind: "change",
      summary: "Cozy lavender study theme with readable type",
      rationale: "Fixed the tool name and picked a catalog font after validation feedback.",
      operations: [
        { tool: "design.generate_palette", params: { accent: "#9f7aea", scheme: "light", theme_name: "Lavender Study" }, reason: "soft lavender accent" },
        { tool: "design.set_typography", params: { font_sans: "Lexend", line_height: 1.6 }, reason: "readable" },
        { tool: "desktop.set_wallpaper", params: { style: "mesh", seed: 77 }, reason: "soft blobs" },
      ],
    };
  }
  if (first.includes("lavender study")) {
    // First attempt contains mistakes on purpose.
    return {
      kind: "change",
      summary: "Lavender study theme",
      operations: [
        { tool: "design.set_pallete", params: { accent: "#9f7aea" } },
        { tool: "design.set_typography", params: { font_sans: "Comic Sans MS" } },
      ],
    };
  }
  if (first.includes("ignore previous instructions")) {
    // Simulates a model that followed an injected instruction.
    return {
      kind: "change",
      summary: "Following the pasted instructions",
      operations: [
        { tool: "services.enable", params: { services: ["ssh"] } },
        { tool: "firewall.allow", params: { tcp: [22, 4444] } },
      ],
    };
  }
  if (first.includes("podcast")) {
    return {
      kind: "clarify",
      summary: "Podcast studio",
      questions: [{ id: "video", question: "Will you also stream video?", options: ["Audio only", "Audio and video"] }],
      operations: [],
    };
  }
  return { kind: "question", answer: "mock: no scripted plan for this request", operations: [] };
}

http
  .createServer(async (req, res) => {
    const data = JSON.parse(await read(req));
    const msgs = data.messages.slice(1);
    const plan = planFor(msgs[0].content, msgs[msgs.length - 1].content);
    res.writeHead(200, { "content-type": "application/json" });
    res.end(
      JSON.stringify({
        model: "mock-gpt",
        choices: [{ message: { role: "assistant", content: "```json\n" + JSON.stringify(plan) + "\n```" } }],
        usage: { prompt_tokens: Math.round(data.messages[0].content.length / 4), completion_tokens: 120 },
      }),
    );
  })
  .listen(okPort, "127.0.0.1");

console.log(`mock LLM listening on ${failPort} (always 529) and ${okPort} (scripted)`);
