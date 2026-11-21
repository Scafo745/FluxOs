//! HTTP API (JSON + SSE). Bound to 127.0.0.1 by default.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use flux_core::policy::Decision;
use flux_core::{SystemSpec, catalog, registry};
use futures::Stream;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth::Principal;
use crate::engine::{Engine, EngineError, Fault, ProposeRequest};
use crate::store::{PlanKind, PlanStatus};

pub type AppState = Arc<Engine>;

impl IntoResponse for EngineError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(json!({ "error": self }))).into_response()
    }
}

/// Authenticated caller. Accepts `Authorization: Bearer <token>` or, for
/// EventSource clients, `?access_token=<token>`.
pub struct Auth(pub Principal);

impl FromRequestParts<AppState> for Auth {
    type Rejection = EngineError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::to_string);
        let query = parts
            .uri
            .query()
            .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("access_token=")).map(str::to_string));
        let token = header.or(query).ok_or_else(|| EngineError::new(401, "unauthorized", "missing bearer token"))?;
        state
            .tokens
            .authenticate(&token)
            .map(Auth)
            .ok_or_else(|| EngineError::new(401, "unauthorized", "invalid token"))
    }
}

pub fn router(engine: AppState, origins: &[String]) -> Router {
    let origins: Vec<HeaderValue> = origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::HeaderName::from_static("last-event-id")])
        .max_age(Duration::from_secs(600));

    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/boot", get(boot_report))
        .route("/v1/theme.css", get(theme_css))
        .route("/v1/tokens.json", get(tokens_json))
        .route("/v1/wallpaper.svg", get(wallpaper))
        .route("/v1/events", get(events))
        .route("/v1/whoami", get(whoami))
        .route("/v1/system", get(system))
        .route("/v1/system/reboot", post(reboot))
        .route("/v1/hardware", get(hardware))
        .route("/v1/tools", get(tools))
        .route("/v1/catalog", get(catalog_all))
        .route("/v1/schema", get(schema))
        .route("/v1/profiles", get(list_profiles).post(save_profile))
        .route("/v1/profiles/import", post(import_profile))
        .route("/v1/profiles/{id}", get(get_profile))
        .route("/v1/profiles/{id}/export", get(export_profile))
        .route("/v1/plans", get(list_plans).post(propose))
        .route("/v1/plans/validate", post(validate))
        .route("/v1/plans/{id}", get(get_plan))
        .route("/v1/plans/{id}/approve", post(approve))
        .route("/v1/plans/{id}/apply", post(apply))
        .route("/v1/plans/{id}/reject", post(reject))
        .route("/v1/plans/{id}/preview", get(plan_preview))
        .route("/v1/generations", get(generations))
        .route("/v1/generations/{n}", get(generation))
        .route("/v1/generations/{n}/file", get(generation_file))
        .route("/v1/rollback", post(rollback))
        .route("/v1/audit", get(audit))
        .route("/v1/audit/verify", get(audit_verify))
        .route("/v1/dev/fault", post(dev_fault))
        .route("/mcp", post(crate::mcp::handle))
        .layer(DefaultBodyLimit::max(1 << 20))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(engine)
}

type R<T> = Result<T, EngineError>;

// ------------------------------------------------------------- public --

async fn health(State(e): State<AppState>) -> Json<Value> {
    let degraded = e.degraded_reasons();
    Json(json!({
        "status": if degraded.is_empty() { "ok" } else { "degraded" },
        "version": e.version,
        "generation": e.current_generation(),
        "target": e.target.name(),
        "mode": e.mode,
        "uptime_s": e.started.elapsed().as_secs(),
        "degraded": degraded,
        "ready": e.boot.read().map(|b| b.ready).unwrap_or(false),
    }))
}

async fn boot_report(State(e): State<AppState>) -> Json<Value> {
    Json(serde_json::to_value(&*e.boot.read().expect("boot lock")).unwrap_or_default())
}

fn current_artifact(e: &Engine, path: &str) -> R<String> {
    e.store
        .read_artifact(e.current_generation(), path)
        .map_err(|_| EngineError::not_found(format!("{path} not rendered")))
}

fn text(content_type: &'static str, body: String) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

async fn theme_css(State(e): State<AppState>) -> R<Response> {
    Ok(text("text/css; charset=utf-8", current_artifact(&e, "etc/fluxos/theme.css")?))
}

async fn tokens_json(State(e): State<AppState>) -> R<Response> {
    Ok(text("application/design-tokens+json", current_artifact(&e, "etc/fluxos/tokens.json")?))
}

async fn wallpaper(State(e): State<AppState>) -> R<Response> {
    Ok(text("image/svg+xml", current_artifact(&e, "etc/fluxos/wallpaper.svg")?))
}

// -------------------------------------------------------------- events --

#[derive(Deserialize)]
struct EventsQuery {
    since: Option<u64>,
}

async fn events(
    State(e): State<AppState>,
    _auth: Auth,
    headers: HeaderMap,
    Query(q): Query<EventsQuery>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let since = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .or(q.since)
        .unwrap_or(0);
    let (replay, rx) = e.events.subscribe(since);
    let last = replay.last().map(|ev| ev.seq).unwrap_or(since);
    let live = BroadcastStream::new(rx).filter_map(|r| r.ok()).filter(move |ev| ev.seq > last);
    let stream = tokio_stream::iter(replay).chain(live).map(|ev| {
        Ok(SseEvent::default()
            .id(ev.seq.to_string())
            .event(ev.kind.clone())
            .json_data(&ev)
            .unwrap_or_else(|_| SseEvent::default().data("{}")))
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

// -------------------------------------------------------------- system --

async fn whoami(State(e): State<AppState>, Auth(who): Auth) -> Json<Value> {
    let tiers: Vec<u8> =
        (0..=3u8).filter(|t| flux_core::RiskTier::from_u8(*t).is_some_and(|t| who.can_approve(t, e.mode))).collect();
    Json(json!({ "id": who.id, "role": who.role, "mode": e.mode, "can_approve_tiers": tiers }))
}

async fn system(State(e): State<AppState>, _auth: Auth) -> R<Json<Value>> {
    let n = e.current_generation();
    let spec = e.current_spec()?;
    let meta = e.store.load_meta(n)?;
    let pending = e
        .store
        .list_plans(100)?
        .iter()
        .filter(|p| {
            matches!(p.status, PlanStatus::Proposed | PlanStatus::Approved) && p.decision != Decision::AutoApply
        })
        .count();
    Ok(Json(json!({
        "generation": n,
        "status": meta.status,
        "theme_name": spec.design.theme_name,
        "spec_digest": spec.digest(),
        "spec": spec,
        "target": e.target.describe(),
        "mode": e.mode,
        "hardware": e.hardware.summary(),
        "degraded": e.degraded_reasons(),
        "pending_approvals": pending,
        "audit_head": e.audit.head(),
    })))
}

async fn reboot(State(e): State<AppState>, Auth(who): Auth) -> R<Json<Value>> {
    Ok(Json(e.reboot(&who).await?))
}

async fn hardware(State(e): State<AppState>, _auth: Auth) -> Json<Value> {
    Json(serde_json::to_value(&e.hardware).unwrap_or_default())
}

async fn tools(_auth: Auth) -> Json<Value> {
    Json(json!({ "default_policy": "deny", "tools": registry::infos() }))
}

async fn catalog_all(_auth: Auth) -> Json<Value> {
    Json(json!({
        "packages": catalog::PACKAGES,
        "services": catalog::SERVICES,
        "fonts": catalog::FONTS,
        "icon_themes": catalog::ICON_THEMES,
        "cursor_themes": catalog::CURSOR_THEMES,
        "kernels": catalog::KERNELS,
        "schedulers": catalog::SCHEDULERS,
        "sysctls": catalog::SYSCTLS,
        "splash_themes": catalog::SPLASH_BUILTIN.iter().chain(catalog::SPLASH_ADI1090X).collect::<Vec<_>>(),
    }))
}

pub fn plan_request_schema() -> Value {
    let tools: Vec<&str> = registry::TOOLS.iter().filter(|t| t.blocked_reason.is_none()).map(|t| t.name).collect();
    json!({
        "type": "object",
        "required": ["operations"],
        "properties": {
            "intent": { "type": "object", "properties": {
                "prompt": { "type": "string" }, "source": { "type": "string" }, "session_id": { "type": "string" }
            }},
            "summary": { "type": "string", "description": "Model's short description (untrusted, shown separately)" },
            "rationale": { "type": "string" },
            "engine": { "type": "object" },
            "operations": { "type": "array", "maxItems": flux_core::policy::MAX_OPERATIONS, "items": {
                "type": "object",
                "required": ["tool", "params"],
                "properties": {
                    "tool": { "type": "string", "enum": tools },
                    "params": { "type": "object" },
                    "reason": { "type": "string" }
                }
            }}
        }
    })
}

async fn schema(_auth: Auth) -> Json<Value> {
    Json(json!({
        "version": flux_core::SCHEMA_VERSION,
        "plan_request": plan_request_schema(),
        "spec": registry::schema_of::<SystemSpec>(),
    }))
}

// ------------------------------------------------------------ profiles --

async fn list_profiles(State(e): State<AppState>, _auth: Auth) -> Json<Value> {
    let profiles = e.profiles.read().expect("profiles lock");
    Json(json!({ "profiles": profiles.iter().map(|p| p.summary_json()).collect::<Vec<_>>() }))
}

async fn get_profile(State(e): State<AppState>, _auth: Auth, Path(id): Path<String>) -> R<Json<Value>> {
    let profiles = e.profiles.read().expect("profiles lock");
    let p = profiles.get(&id).ok_or_else(|| EngineError::not_found(format!("profile {id} not found")))?;
    let mut v = p.summary_json();
    v["description"] = json!(p.doc.description);
    v["spec"] = serde_json::to_value(&p.spec).unwrap_or_default();
    Ok(Json(v))
}

#[derive(Deserialize)]
struct SaveProfile {
    id: String,
    name: String,
    summary: Option<String>,
}

async fn save_profile(State(e): State<AppState>, Auth(who): Auth, Json(b): Json<SaveProfile>) -> R<Json<Value>> {
    Ok(Json(e.save_profile(&who, &b.id, &b.name, b.summary)?))
}

async fn export_profile(State(e): State<AppState>, _auth: Auth, Path(id): Path<String>) -> R<Json<Value>> {
    Ok(Json(e.export_profile(&id)?))
}

async fn import_profile(State(e): State<AppState>, Auth(who): Auth, Json(bundle): Json<Value>) -> R<Json<Value>> {
    Ok(Json(e.import_profile(&who, bundle)?))
}

// --------------------------------------------------------------- plans --

#[derive(Deserialize)]
struct ProposeQuery {
    /// Apply immediately when the policy decision is auto_apply (tier 0-1).
    #[serde(default)]
    auto_apply: bool,
}

async fn propose(
    State(e): State<AppState>,
    Auth(who): Auth,
    Query(q): Query<ProposeQuery>,
    Json(req): Json<ProposeRequest>,
) -> R<Json<Value>> {
    let plan = e.propose(&who, req, PlanKind::Change, false)?;
    if q.auto_apply && plan.status == PlanStatus::Proposed && plan.decision == Decision::AutoApply {
        let applied = e.apply(&who, &plan.id).await?;
        return Ok(Json(serde_json::to_value(applied).unwrap_or_default()));
    }
    Ok(Json(serde_json::to_value(plan).unwrap_or_default()))
}

async fn validate(State(e): State<AppState>, Auth(who): Auth, Json(req): Json<ProposeRequest>) -> R<Json<Value>> {
    let plan = e.propose(&who, req, PlanKind::Change, true)?;
    Ok(Json(serde_json::to_value(plan).unwrap_or_default()))
}

#[derive(Deserialize)]
struct ListQuery {
    limit: Option<usize>,
    status: Option<String>,
}

async fn list_plans(State(e): State<AppState>, _auth: Auth, Query(q): Query<ListQuery>) -> R<Json<Value>> {
    let plans = e.store.list_plans(q.limit.unwrap_or(50).min(500))?;
    let plans: Vec<Value> = plans
        .into_iter()
        .filter(|p| {
            q.status.as_deref().is_none_or(|s| {
                serde_json::to_value(p.status).ok().and_then(|v| v.as_str().map(|x| x == s)).unwrap_or(false)
            })
        })
        .map(|p| serde_json::to_value(p).unwrap_or_default())
        .collect();
    Ok(Json(json!({ "plans": plans })))
}

async fn get_plan(State(e): State<AppState>, _auth: Auth, Path(id): Path<String>) -> R<Json<Value>> {
    Ok(Json(serde_json::to_value(e.get_plan(&id)?).unwrap_or_default()))
}

#[derive(Deserialize)]
struct PreviewQuery {
    path: Option<String>,
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("json") => "application/json",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        _ => "text/plain; charset=utf-8",
    }
}

/// Renders what a pending plan would produce, so approvers can see the new
/// theme, wallpaper or NixOS module before anything is applied.
async fn plan_preview(
    State(e): State<AppState>,
    _auth: Auth,
    Path(id): Path<String>,
    Query(q): Query<PreviewQuery>,
) -> R<Response> {
    let plan = e.get_plan(&id)?;
    let current = e.current_spec()?;
    let eval = e.evaluate(&current, &plan.operations);
    let candidate =
        eval.candidate.ok_or_else(|| EngineError::conflict("plan does not validate against the current system"))?;
    let next = e.store.next_generation()?;
    let artifacts = flux_core::render::render_all(
        &candidate,
        &flux_core::render::RenderContext { generation: next, plan_id: Some(&id) },
    );
    match q.path {
        Some(path) => {
            let a = artifacts
                .iter()
                .find(|a| a.path == path)
                .ok_or_else(|| EngineError::not_found("no such artifact in the preview"))?;
            Ok(text(content_type(&path), a.content.clone()))
        }
        None => Ok(Json(json!({
            "plan_id": id,
            "generation": next,
            "spec": candidate,
            "artifacts": artifacts.iter().map(crate::store::ArtifactInfo::from).collect::<Vec<_>>(),
        }))
        .into_response()),
    }
}

#[derive(Deserialize)]
struct ApproveBody {
    plan_hash: String,
    #[serde(default = "yes")]
    apply: bool,
}

fn yes() -> bool {
    true
}

async fn approve(
    State(e): State<AppState>,
    Auth(who): Auth,
    Path(id): Path<String>,
    Json(b): Json<ApproveBody>,
) -> R<Json<Value>> {
    Ok(Json(serde_json::to_value(e.approve(&who, &id, &b.plan_hash, b.apply).await?).unwrap_or_default()))
}

async fn apply(State(e): State<AppState>, Auth(who): Auth, Path(id): Path<String>) -> R<Json<Value>> {
    Ok(Json(serde_json::to_value(e.apply(&who, &id).await?).unwrap_or_default()))
}

#[derive(Deserialize, Default)]
struct RejectBody {
    reason: Option<String>,
}

async fn reject(
    State(e): State<AppState>,
    Auth(who): Auth,
    Path(id): Path<String>,
    body: Option<Json<RejectBody>>,
) -> R<Json<Value>> {
    let reason = body.and_then(|Json(b)| b.reason);
    Ok(Json(serde_json::to_value(e.reject(&who, &id, reason)?).unwrap_or_default()))
}

// --------------------------------------------------------- generations --

async fn generations(State(e): State<AppState>, _auth: Auth) -> R<Json<Value>> {
    Ok(Json(json!({ "current": e.current_generation(), "generations": e.generations()? })))
}

async fn generation(State(e): State<AppState>, _auth: Auth, Path(n): Path<u64>) -> R<Json<Value>> {
    let meta = e.store.load_meta(n).map_err(|_| EngineError::not_found(format!("generation {n} not found")))?;
    let spec = e.store.load_spec(n)?;
    Ok(Json(json!({ "meta": meta, "spec": spec })))
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

async fn generation_file(
    State(e): State<AppState>,
    _auth: Auth,
    Path(n): Path<u64>,
    Query(q): Query<FileQuery>,
) -> R<Response> {
    let content =
        e.store.read_artifact(n, &q.path).map_err(|_| EngineError::not_found("file not found in generation"))?;
    Ok(text(content_type(&q.path), content))
}

#[derive(Deserialize, Default)]
struct RollbackBody {
    to: Option<u64>,
    prompt: Option<String>,
}

async fn rollback(State(e): State<AppState>, Auth(who): Auth, body: Option<Json<RollbackBody>>) -> R<Json<Value>> {
    let b = body.map(|Json(b)| b).unwrap_or_default();
    Ok(Json(serde_json::to_value(e.rollback(&who, b.to, b.prompt).await?).unwrap_or_default()))
}

// --------------------------------------------------------------- audit --

#[derive(Deserialize)]
struct AuditQuery {
    before: Option<u64>,
    limit: Option<usize>,
    kind: Option<String>,
}

async fn audit(State(e): State<AppState>, _auth: Auth, Query(q): Query<AuditQuery>) -> R<Json<Value>> {
    let entries = e.audit.page(q.before, q.limit.unwrap_or(50).min(500), q.kind.as_deref())?;
    Ok(Json(json!({ "head": e.audit.head(), "entries": entries })))
}

async fn audit_verify(State(e): State<AppState>, _auth: Auth) -> R<Json<Value>> {
    Ok(Json(serde_json::to_value(crate::audit::verify(e.audit.path())?).unwrap_or_default()))
}

// ----------------------------------------------------------------- dev --

#[derive(Deserialize)]
struct FaultBody {
    fault: Option<Fault>,
}

async fn dev_fault(State(e): State<AppState>, Auth(who): Auth, Json(b): Json<FaultBody>) -> R<Json<Value>> {
    if !e.dev {
        return Err(EngineError::not_found("developer endpoints are disabled (start fluxd with --dev)"));
    }
    if !who.can_use_dev_tools() {
        return Err(EngineError::forbidden("admin only"));
    }
    e.set_fault(b.fault);
    Ok(Json(json!({ "armed": b.fault })))
}
