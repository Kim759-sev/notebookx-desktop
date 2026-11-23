use crate::state::AppCore;
use crate::types::{SessionLifecycle, WorkflowRunRequest};
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct GatewayState {
    core: Arc<AppCore>,
}

pub async fn start_local_gateway(core: Arc<AppCore>, port: u16) -> Result<SocketAddr, String> {
    let state = GatewayState { core };
    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/api/v1/status", get(handle_status))
        .route("/api/v1/workflows", get(handle_list_workflows))
        .route("/api/v1/wake", post(handle_wake))
        .route("/api/v1/stop", post(handle_stop))
        .route("/api/v1/comfy/run", post(handle_run_workflow))
        .route("/management/*rest", axum::routing::any(proxy_management))
        .fallback(proxy_comfy)
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind local API gateway on {addr}: {e}"))?;
    let bound_addr = listener
        .local_addr()
        .map_err(|e| format!("Failed to query bound gateway addr: {e}"))?;

    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            tracing::error!("Local API gateway exited with error: {err}");
        }
    });

    Ok(bound_addr)
}

async fn handle_health(State(st): State<GatewayState>) -> impl IntoResponse {
    let snap = st.core.snapshot().await;
    let active = snap.sessions.iter().find(|s| {
        matches!(
            s.state,
            SessionLifecycle::Ready | SessionLifecycle::Busy | SessionLifecycle::Bootstrapping
        )
    });
    Json(json!({
        "ok": true,
        "service": "notebookx-desktop-gateway",
        "connected": active.map(|s| s.comfyReady).unwrap_or(false),
        "stage": active.map(|s| format!("{:?}", s.state).to_lowercase()).unwrap_or_else(|| "stopped".to_string()),
        "activeSession": active.map(|s| &s.name),
        "gpu": active.and_then(|s| s.gpuName.as_ref()),
        "vramTotalMib": active.and_then(|s| s.vramTotalMib),
        "driveMounted": active.map(|s| s.driveMounted).unwrap_or(false),
    }))
}

async fn handle_status(State(st): State<GatewayState>) -> impl IntoResponse {
    let snap = st.core.snapshot().await;
    let active_account = snap.accounts.iter().find(|a| a.isActive);
    let active_session = snap.sessions.iter().find(|s| {
        !matches!(s.state, SessionLifecycle::Stopped | SessionLifecycle::Error)
    });
    Json(json!({
        "gatewayUrl": snap.env.gatewayUrl,
        "backendMode": snap.settings.backendMode,
        "autoWakeOnApiCall": snap.settings.autoWakeOnApiCall,
        "account": active_account,
        "activeSession": active_session,
        "tunnels": snap.tunnels,
    }))
}

async fn handle_list_workflows(State(st): State<GatewayState>) -> impl IntoResponse {
    let snap = st.core.snapshot().await;
    Json(snap.apiWorkflows)
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WakeBody {
    profile_id: Option<String>,
}

async fn handle_wake(
    State(st): State<GatewayState>,
    body: Option<Json<WakeBody>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let profile_id = body
        .and_then(|b| b.0.profile_id)
        .unwrap_or_else(|| "dramasmith-h3-bf16".to_string());
    match st.core.ensure_or_launch_profile(&profile_id).await {
        Ok(session) => Ok(Json(json!({ "ok": true, "session": session }))),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "ok": false, "error": err })),
        )),
    }
}

#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StopBody {
    session_name: Option<String>,
    #[serde(default)]
    force: bool,
}

async fn handle_stop(
    State(st): State<GatewayState>,
    body: Option<Json<StopBody>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let payload = body.map(|b| b.0).unwrap_or_default();
    let target_name = if let Some(n) = payload.session_name {
        n
    } else {
        let snap = st.core.snapshot().await;
        match snap
            .sessions
            .iter()
            .find(|s| !matches!(s.state, SessionLifecycle::Stopped))
        {
            Some(s) => s.name.clone(),
            None => {
                return Ok(Json(
                    json!({ "ok": true, "message": "No active session running" }),
                ))
            }
        }
    };

    match st.core.stop_session(&target_name, payload.force).await {
        Ok(session) => Ok(Json(json!({
            "ok": true,
            "verifiedUnassigned": session.verifiedUnassigned,
            "session": session
        }))),
        Err(err) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "ok": false, "error": err })),
        )),
    }
}

async fn handle_run_workflow(
    State(st): State<GatewayState>,
    Json(req): Json<WorkflowRunRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    st.core.touch_activity().await;
    match st.core.run_api_workflow(req).await {
        Ok(res) => Ok(Json(res)),
        Err(err) => Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({ "ok": false, "error": err })),
        )),
    }
}

async fn proxy_management(
    State(st): State<GatewayState>,
    Path(rest): Path<String>,
    req: Request,
) -> Response {
    let port = st.core.active_management_local_port().await.unwrap_or(8189);
    forward_http_request(&st.core.local_http_client, port, &format!("/{rest}"), req).await
}

async fn proxy_comfy(State(st): State<GatewayState>, req: Request) -> Response {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    // Auto-wake on /prompt POST when configured and no session is ready yet
    if req.method() == axum::http::Method::POST && path_and_query.starts_with("/prompt") {
        st.core.touch_activity().await;
        let (auto_wake, has_ready, default_profile) = st.core.auto_wake_check_info().await;
        if auto_wake && !has_ready {
            if let Err(err) = st.core.ensure_or_launch_profile(&default_profile).await {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({ "error": format!("Auto-wake failed: {err}") })),
                )
                    .into_response();
            }
        }
    }

    let port = st.core.active_comfy_local_port().await.unwrap_or(8188);
    forward_http_request(&st.core.local_http_client, port, &path_and_query, req).await
}

async fn forward_http_request(
    client: &reqwest::Client,
    local_port: u16,
    path_and_query: &str,
    req: Request,
) -> Response {
    let target_url = format!("http://127.0.0.1:{local_port}{path_and_query}");
    let method = match reqwest::Method::from_bytes(req.method().as_str().as_bytes()) {
        Ok(m) => m,
        Err(_) => return StatusCode::METHOD_NOT_ALLOWED.into_response(),
    };

    let req_headers: HeaderMap = req.headers().clone();
    let body_bytes = match axum::body::to_bytes(req.into_body(), 64 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Invalid request body: {e}") })),
            )
                .into_response()
        }
    };

    let mut builder = client.request(method, &target_url);
    for (name, val) in req_headers.iter() {
        let key = name.as_str();
        if !matches!(key, "host" | "content-length" | "connection") {
            if let Ok(v) = val.to_str() {
                builder = builder.header(key, v);
            }
        }
    }

    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    match builder.send().await {
        Ok(upstream) => {
            let status =
                StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::OK);
            let mut resp_builder = Response::builder().status(status);
            for (k, v) in upstream.headers().iter() {
                let key = k.as_str();
                if !matches!(key, "transfer-encoding" | "connection") {
                    resp_builder = resp_builder.header(key, v.as_bytes());
                }
            }
            match upstream.bytes().await {
                Ok(bytes) => resp_builder
                    .body(Body::from(bytes))
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "error": format!("Upstream body error: {e}") })),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "Cloud ComfyUI tunnel is not connected yet",
                "target": target_url,
                "detail": e.to_string()
            })),
        )
            .into_response(),
    }
}
