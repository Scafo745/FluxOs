//! Minimal MCP server (JSON-RPC 2.0 over Streamable HTTP, JSON responses).
//!
//! External AI clients see the same registry as the FluxOS agent. A tool
//! call becomes a one-operation plan that goes through the full policy
//! pipeline, so MCP clients get no extra privileges: tier 0-1 applies,
//! tier 2-3 waits for a person, tier 4 is refused.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use flux_core::policy::Decision;
use flux_core::{RawOp, RiskTier, registry};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{AppState, Auth};
use crate::engine::ProposeRequest;
use crate::store::{Intent, PlanKind, PlanStatus};

const PROTOCOL: &str = "2025-06-18";

#[derive(Deserialize)]
pub struct RpcRequest {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

fn ok(id: Value, result: Value) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result })).into_response()
}

fn err(id: Value, code: i64, message: &str) -> Response {
    Json(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })).into_response()
}

fn mcp_name(tool: &str) -> String {
    tool.replace('.', "_")
}

fn tool_list() -> Value {
    let mut tools: Vec<Value> = registry::TOOLS
        .iter()
        .filter(|t| t.blocked_reason.is_none())
        .map(|t| {
            json!({
                "name": mcp_name(t.name),
                "title": t.title,
                "description": format!("{} [FluxOS risk tier {}: {}]", t.description, t.tier.as_u8(), t.tier.behavior()),
                "inputSchema": (t.schema)(),
                "annotations": {
                    "title": t.title,
                    "readOnlyHint": false,
                    "destructiveHint": t.tier >= RiskTier::Medium,
                    "idempotentHint": false,
                    "openWorldHint": false
                }
            })
        })
        .collect();
    tools.push(json!({
        "name": "fluxos_get_system",
        "title": "Current system",
        "description": "Current generation, theme, desktop, kernel and pending approvals.",
        "inputSchema": { "type": "object", "properties": {} },
        "annotations": { "readOnlyHint": true, "openWorldHint": false }
    }));
    json!({ "tools": tools })
}

fn text_result(text: String, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

pub async fn handle(State(e): State<AppState>, Auth(who): Auth, Json(req): Json<RpcRequest>) -> Response {
    let Some(id) = req.id.clone() else {
        // Notifications (e.g. notifications/initialized) get no body.
        return StatusCode::ACCEPTED.into_response();
    };
    match req.method.as_str() {
        "initialize" => {
            let requested = req.params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL);
            let version =
                if ["2025-06-18", "2025-03-26", "2024-11-05"].contains(&requested) { requested } else { PROTOCOL };
            ok(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "fluxd", "title": "FluxOS", "version": e.version },
                    "instructions": "Every tool call becomes a validated FluxOS plan. Tier 0-1 changes apply with a snapshot; tier 2-3 wait for human approval in FluxOS; tier 4 is never allowed."
                }),
            )
        }
        "ping" => ok(id, json!({})),
        "tools/list" => ok(id, tool_list()),
        "tools/call" => {
            let name = req.params.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
            let args = req.params.get("arguments").cloned().unwrap_or(json!({}));
            if name == "fluxos_get_system" {
                let spec = match e.current_spec() {
                    Ok(s) => s,
                    Err(x) => return ok(id, text_result(x.message, true)),
                };
                let text = format!(
                    "Generation {} · theme {} · {:?} desktop ({:?}) · kernel {:?} · scheduler {:?}",
                    e.current_generation(),
                    spec.design.theme_name,
                    spec.desktop.environment,
                    spec.desktop.style,
                    spec.kernel.variant,
                    spec.kernel.sched_ext
                );
                return ok(id, text_result(text, false));
            }
            let Some(tool) = registry::TOOLS.iter().find(|t| mcp_name(t.name) == name && t.blocked_reason.is_none())
            else {
                return ok(id, text_result(format!("Unknown tool {name:?}. FluxOS tools are default-deny."), true));
            };
            let plan_req = ProposeRequest {
                intent: Intent {
                    prompt: Some(format!("MCP call {}", tool.name)),
                    source: Some("mcp".into()),
                    session_id: None,
                },
                summary: None,
                rationale: None,
                engine: json!({ "kind": "mcp" }),
                operations: vec![RawOp { tool: tool.name.to_string(), params: args, reason: None }],
            };
            let plan = match e.propose(&who, plan_req, PlanKind::Change, false) {
                Ok(p) => p,
                Err(x) => return ok(id, text_result(x.message, true)),
            };
            if !plan.evaluation.ok {
                let issues: Vec<String> = plan
                    .evaluation
                    .issues
                    .iter()
                    .filter(|i| i.is_error())
                    .map(|i| format!("{}: {}", i.path, i.message))
                    .collect();
                return ok(
                    id,
                    text_result(format!("Rejected by FluxOS policy ({:?}): {}", plan.status, issues.join("; ")), true),
                );
            }
            if plan.status == PlanStatus::NoOp {
                return ok(id, text_result("No change needed: the system already matches.".into(), false));
            }
            if plan.status == PlanStatus::Proposed && plan.decision == Decision::AutoApply {
                return match e.apply(&who, &plan.id).await {
                    Ok(p) => {
                        let msg = p.result.as_ref().map(|r| r.message.clone()).unwrap_or_default();
                        ok(
                            id,
                            text_result(
                                format!("{msg}\n{}", p.change_summary),
                                p.status != PlanStatus::Applied && p.status != PlanStatus::Staged,
                            ),
                        )
                    }
                    Err(x) => ok(id, text_result(x.message, true)),
                };
            }
            ok(
                id,
                text_result(
                    format!(
                        "Plan {} needs approval (tier {}, {:?}). A person must approve it in FluxOS.\n{}",
                        plan.id,
                        plan.evaluation.tier.as_u8(),
                        plan.decision,
                        plan.change_summary
                    ),
                    false,
                ),
            )
        }
        _ => err(id, -32601, "method not found"),
    }
}
