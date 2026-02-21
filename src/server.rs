use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

use crate::case::*;
use crate::export::network_to_raw;
use crate::parse::parse_raw_str;

static INDEX_HTML: &str = include_str!("../static/index.html");

type AppState = Arc<Mutex<Option<Network>>>;

// Helper: respond with JSON
fn json_ok(val: serde_json::Value) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        val.to_string(),
    )
        .into_response()
}

fn json_err(status: StatusCode, msg: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        json!({"error": msg}).to_string(),
    )
        .into_response()
}

// GET /logo.png  — serve logo from static/ directory at runtime (optional file)
async fn serve_logo() -> Response {
    match std::fs::read("static/mantis16.png") {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/png")],
            Body::from(bytes),
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

// GET /
async fn serve_index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        INDEX_HTML,
    )
}

// POST /api/upload  multipart/form-data with field "file"
async fn upload_raw(State(state): State<AppState>, mut multipart: Multipart) -> Response {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = match field.bytes().await {
                Ok(b) => b,
                Err(_) => return json_err(StatusCode::BAD_REQUEST, "Failed to read file bytes"),
            };
            let content = String::from_utf8_lossy(&bytes).to_string();
            let mut network = parse_raw_str(&content);
            network.rebuild_bus_map();
            let net_json = serde_json::to_value(&network).unwrap_or(json!(null));
            *state.lock().await = Some(network);
            return json_ok(net_json);
        }
    }
    json_err(StatusCode::BAD_REQUEST, "No 'file' field in multipart form")
}

// GET /api/network
async fn get_network(State(state): State<AppState>) -> Response {
    let guard = state.lock().await;
    match guard.as_ref() {
        Some(net) => json_ok(serde_json::to_value(net).unwrap_or(json!(null))),
        None => json_err(StatusCode::NOT_FOUND, "No network loaded"),
    }
}

// PUT /api/network  — replace entire network
async fn put_network(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    match serde_json::from_value::<Network>(body.0) {
        Ok(mut net) => {
            net.rebuild_bus_map();
            let net_json = serde_json::to_value(&net).unwrap_or(json!(null));
            *state.lock().await = Some(net);
            json_ok(net_json)
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// POST /api/network/new  — create empty network
#[derive(Deserialize)]
struct NewNetworkBody {
    case_name: String,
    s_base: f32,
    frequency: f32,
}

async fn new_network(
    State(state): State<AppState>,
    body: axum::extract::Json<NewNetworkBody>,
) -> Response {
    let net = Network::new(body.case_name.clone(), body.s_base, body.frequency);
    let net_json = serde_json::to_value(&net).unwrap_or(json!(null));
    *state.lock().await = Some(net);
    json_ok(net_json)
}

// POST /api/buses
async fn add_bus(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let new_id = net
        .buses
        .iter()
        .map(|b| b.bus_id)
        .max()
        .map(|m| m + 1)
        .unwrap_or(1);

    let mut bus_val = body.0;
    bus_val["bus_id"] = json!(new_id);

    match serde_json::from_value::<Bus>(bus_val) {
        Ok(bus) => {
            net.buses.push(bus);
            net.rebuild_bus_map();
            let added = net.buses.last().unwrap();
            json_ok(serde_json::to_value(added).unwrap_or(json!(null)))
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// PUT /api/buses/:id
async fn update_bus(
    State(state): State<AppState>,
    Path(id): Path<usize>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    match net.buses.iter_mut().find(|b| b.bus_id == id) {
        None => json_err(StatusCode::NOT_FOUND, "Bus not found"),
        Some(bus) => match serde_json::from_value::<Bus>(body.0) {
            Ok(updated) => {
                *bus = updated;
                let updated_json = serde_json::to_value(bus).unwrap_or(json!(null));
                net.rebuild_bus_map();
                json_ok(updated_json)
            }
            Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
        },
    }
}

// DELETE /api/buses/:id  — cascade to branches, loads, generators
async fn delete_bus(State(state): State<AppState>, Path(id): Path<usize>) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let orig_len = net.buses.len();
    net.buses.retain(|b| b.bus_id != id);
    if net.buses.len() == orig_len {
        return json_err(StatusCode::NOT_FOUND, "Bus not found");
    }

    // Cascade deletes
    net.branches.retain(|b| b.from_bus != id && b.to_bus != id);
    net.generators.retain(|g| g.gen_bus_id != id);
    net.loads.retain(|l| l.bus_id != id);
    net.fixed_shunts.retain(|s| s.bus_id != id);
    net.switched_shunts.retain(|s| s.bus_id != id);
    net.rebuild_bus_map();

    json_ok(json!({"deleted": id}))
}

// POST /api/branches
async fn add_branch(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let new_id = net
        .branches
        .iter()
        .map(|b| b.id)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);

    let mut branch_val = body.0;
    branch_val["id"] = json!(new_id);

    match serde_json::from_value::<Branch>(branch_val) {
        Ok(branch) => {
            net.branches.push(branch);
            let added = net.branches.last().unwrap();
            json_ok(serde_json::to_value(added).unwrap_or(json!(null)))
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// PUT /api/branches/:id
async fn update_branch(
    State(state): State<AppState>,
    Path(id): Path<usize>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    match net.branches.iter_mut().find(|b| b.id == id) {
        None => json_err(StatusCode::NOT_FOUND, "Branch not found"),
        Some(branch) => match serde_json::from_value::<Branch>(body.0) {
            Ok(updated) => {
                *branch = updated;
                json_ok(serde_json::to_value(branch).unwrap_or(json!(null)))
            }
            Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
        },
    }
}

// DELETE /api/branches/:id
async fn delete_branch(State(state): State<AppState>, Path(id): Path<usize>) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let orig_len = net.branches.len();
    net.branches.retain(|b| b.id != id);
    if net.branches.len() == orig_len {
        return json_err(StatusCode::NOT_FOUND, "Branch not found");
    }
    json_ok(json!({"deleted": id}))
}

// POST /api/generators
async fn add_generator(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let new_id = net
        .generators
        .iter()
        .map(|g| g.gen_id)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);

    let mut gen_val = body.0;
    gen_val["gen_id"] = json!(new_id);

    match serde_json::from_value::<Generator>(gen_val) {
        Ok(generator) => {
            net.generators.push(generator);
            let added = net.generators.last().unwrap();
            json_ok(serde_json::to_value(added).unwrap_or(json!(null)))
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// PUT /api/generators/:id
async fn update_generator(
    State(state): State<AppState>,
    Path(id): Path<usize>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    match net.generators.iter_mut().find(|g| g.gen_id == id) {
        None => json_err(StatusCode::NOT_FOUND, "Generator not found"),
        Some(generator) => match serde_json::from_value::<Generator>(body.0) {
            Ok(updated) => {
                *generator = updated;
                json_ok(serde_json::to_value(generator).unwrap_or(json!(null)))
            }
            Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
        },
    }
}

// DELETE /api/generators/:id
async fn delete_generator(State(state): State<AppState>, Path(id): Path<usize>) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let orig_len = net.generators.len();
    net.generators.retain(|g| g.gen_id != id);
    if net.generators.len() == orig_len {
        return json_err(StatusCode::NOT_FOUND, "Generator not found");
    }
    json_ok(json!({"deleted": id}))
}

// POST /api/loads
async fn add_load(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let new_id = net
        .loads
        .iter()
        .map(|l| l.load_id)
        .max()
        .map(|m| m + 1)
        .unwrap_or(0);

    let mut load_val = body.0;
    load_val["load_id"] = json!(new_id);

    match serde_json::from_value::<Load>(load_val) {
        Ok(load) => {
            net.loads.push(load);
            let added = net.loads.last().unwrap();
            json_ok(serde_json::to_value(added).unwrap_or(json!(null)))
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// PUT /api/loads/:id
async fn update_load(
    State(state): State<AppState>,
    Path(id): Path<usize>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    match net.loads.iter_mut().find(|l| l.load_id == id) {
        None => json_err(StatusCode::NOT_FOUND, "Load not found"),
        Some(load) => match serde_json::from_value::<Load>(body.0) {
            Ok(updated) => {
                *load = updated;
                json_ok(serde_json::to_value(load).unwrap_or(json!(null)))
            }
            Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
        },
    }
}

// DELETE /api/loads/:id
async fn delete_load(State(state): State<AppState>, Path(id): Path<usize>) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let orig_len = net.loads.len();
    net.loads.retain(|l| l.load_id != id);
    if net.loads.len() == orig_len {
        return json_err(StatusCode::NOT_FOUND, "Load not found");
    }
    json_ok(json!({"deleted": id}))
}

// ── Fixed Shunts (index-addressed) ───────────────────────────────────────────

// POST /api/fixed-shunts
async fn add_fixed_shunt(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    match serde_json::from_value::<FixedShunt>(body.0) {
        Ok(shunt) => {
            net.fixed_shunts.push(shunt);
            let added = net.fixed_shunts.last().unwrap();
            json_ok(serde_json::to_value(added).unwrap_or(json!(null)))
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// PUT /api/fixed-shunts/:idx
async fn update_fixed_shunt(
    State(state): State<AppState>,
    Path(idx): Path<usize>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    match net.fixed_shunts.get_mut(idx) {
        None => json_err(StatusCode::NOT_FOUND, "Fixed shunt not found"),
        Some(shunt) => match serde_json::from_value::<FixedShunt>(body.0) {
            Ok(updated) => {
                *shunt = updated;
                json_ok(serde_json::to_value(shunt).unwrap_or(json!(null)))
            }
            Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
        },
    }
}

// DELETE /api/fixed-shunts/:idx
async fn delete_fixed_shunt(State(state): State<AppState>, Path(idx): Path<usize>) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    if idx >= net.fixed_shunts.len() {
        return json_err(StatusCode::NOT_FOUND, "Fixed shunt not found");
    }
    net.fixed_shunts.remove(idx);
    json_ok(json!({"deleted": idx}))
}

// ── Switched Shunts (index-addressed) ────────────────────────────────────────

// POST /api/switched-shunts
async fn add_switched_shunt(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    match serde_json::from_value::<SwitchedShunt>(body.0) {
        Ok(shunt) => {
            net.switched_shunts.push(shunt);
            let added = net.switched_shunts.last().unwrap();
            json_ok(serde_json::to_value(added).unwrap_or(json!(null)))
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// PUT /api/switched-shunts/:idx
async fn update_switched_shunt(
    State(state): State<AppState>,
    Path(idx): Path<usize>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    match net.switched_shunts.get_mut(idx) {
        None => json_err(StatusCode::NOT_FOUND, "Switched shunt not found"),
        Some(shunt) => match serde_json::from_value::<SwitchedShunt>(body.0) {
            Ok(updated) => {
                *shunt = updated;
                json_ok(serde_json::to_value(shunt).unwrap_or(json!(null)))
            }
            Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
        },
    }
}

// DELETE /api/switched-shunts/:idx
async fn delete_switched_shunt(
    State(state): State<AppState>,
    Path(idx): Path<usize>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    if idx >= net.switched_shunts.len() {
        return json_err(StatusCode::NOT_FOUND, "Switched shunt not found");
    }
    net.switched_shunts.remove(idx);
    json_ok(json!({"deleted": idx}))
}

// ── Areas ─────────────────────────────────────────────────────────────────────

// POST /api/areas
async fn add_area(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    let new_id = net.areas.iter().map(|a| a.area_id).max().map(|m| m + 1).unwrap_or(1);
    let mut val = body.0;
    val["area_id"] = json!(new_id);
    match serde_json::from_value::<Area>(val) {
        Ok(area) => {
            net.areas.push(area);
            let added = net.areas.last().unwrap();
            json_ok(serde_json::to_value(added).unwrap_or(json!(null)))
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// PUT /api/areas/:id
async fn update_area(
    State(state): State<AppState>,
    Path(id): Path<usize>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    match net.areas.iter_mut().find(|a| a.area_id == id) {
        None => json_err(StatusCode::NOT_FOUND, "Area not found"),
        Some(area) => match serde_json::from_value::<Area>(body.0) {
            Ok(updated) => {
                *area = updated;
                json_ok(serde_json::to_value(area).unwrap_or(json!(null)))
            }
            Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
        },
    }
}

// DELETE /api/areas/:id
async fn delete_area(State(state): State<AppState>, Path(id): Path<usize>) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    let orig = net.areas.len();
    net.areas.retain(|a| a.area_id != id);
    if net.areas.len() == orig {
        return json_err(StatusCode::NOT_FOUND, "Area not found");
    }
    json_ok(json!({"deleted": id}))
}

// ── Zones ─────────────────────────────────────────────────────────────────────

// POST /api/zones
async fn add_zone(
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    let new_id = net.zones.iter().map(|z| z.zone_id).max().map(|m| m + 1).unwrap_or(1);
    let mut val = body.0;
    val["zone_id"] = json!(new_id);
    match serde_json::from_value::<Zone>(val) {
        Ok(zone) => {
            net.zones.push(zone);
            let added = net.zones.last().unwrap();
            json_ok(serde_json::to_value(added).unwrap_or(json!(null)))
        }
        Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

// PUT /api/zones/:id
async fn update_zone(
    State(state): State<AppState>,
    Path(id): Path<usize>,
    body: axum::extract::Json<serde_json::Value>,
) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    match net.zones.iter_mut().find(|z| z.zone_id == id) {
        None => json_err(StatusCode::NOT_FOUND, "Zone not found"),
        Some(zone) => match serde_json::from_value::<Zone>(body.0) {
            Ok(updated) => {
                *zone = updated;
                json_ok(serde_json::to_value(zone).unwrap_or(json!(null)))
            }
            Err(e) => json_err(StatusCode::BAD_REQUEST, &e.to_string()),
        },
    }
}

// DELETE /api/zones/:id
async fn delete_zone(State(state): State<AppState>, Path(id): Path<usize>) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };
    let orig = net.zones.len();
    net.zones.retain(|z| z.zone_id != id);
    if net.zones.len() == orig {
        return json_err(StatusCode::NOT_FOUND, "Zone not found");
    }
    json_ok(json!({"deleted": id}))
}

// POST /api/solve/dc
async fn solve_dc(State(state): State<AppState>) -> Response {
    let mut guard = state.lock().await;
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    match net.dc_approximation() {
        Ok(()) => {
            let net_json = serde_json::to_value(&net).unwrap_or(json!(null));
            json_ok(net_json)
        },
        Err(msg) => json_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            &msg,
        ),
    }
}

// POST /api/solve/nr
async fn solve_nr(State(state): State<AppState>) -> Response {
    let mut guard = state.lock().await; // Acquire mutable lock
    let net = match guard.as_mut() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let sol = net.newton_raphson_solution(); // newton_raphson_solution now returns AcSolution directly and updates `net`
    // The `sol` object contains the log and the results for frontend display.
    // The `net` object is also updated in place.

    json_ok(serde_json::to_value(&sol).unwrap_or(json!(null)))
}

// GET /api/export
async fn export_raw(State(state): State<AppState>) -> Response {
    let guard = state.lock().await;
    let net = match guard.as_ref() {
        Some(n) => n,
        None => return json_err(StatusCode::NOT_FOUND, "No network loaded"),
    };

    let raw_content = network_to_raw(net);
    let filename = format!("{}.raw", net.case_name.replace(' ', "_"));

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "text/plain; charset=utf-8".parse().unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", filename)
            .parse()
            .unwrap(),
    );

    (StatusCode::OK, headers, Body::from(raw_content)).into_response()
}

pub async fn run_server() {
    let state: AppState = Arc::new(Mutex::new(None));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/logo.png", get(serve_logo))
        .route("/api/upload", post(upload_raw))
        .route("/api/network", get(get_network).put(put_network))
        .route("/api/network/new", post(new_network))
        .route("/api/buses", post(add_bus))
        .route("/api/buses/{id}", put(update_bus).delete(delete_bus))
        .route("/api/branches", post(add_branch))
        .route(
            "/api/branches/{id}",
            put(update_branch).delete(delete_branch),
        )
        .route("/api/generators", post(add_generator))
        .route(
            "/api/generators/{id}",
            put(update_generator).delete(delete_generator),
        )
        .route("/api/loads", post(add_load))
        .route("/api/loads/{id}", put(update_load).delete(delete_load))
        .route("/api/fixed-shunts", post(add_fixed_shunt))
        .route(
            "/api/fixed-shunts/{idx}",
            put(update_fixed_shunt).delete(delete_fixed_shunt),
        )
        .route("/api/switched-shunts", post(add_switched_shunt))
        .route(
            "/api/switched-shunts/{idx}",
            put(update_switched_shunt).delete(delete_switched_shunt),
        )
        .route("/api/areas", post(add_area))
        .route("/api/areas/{id}", put(update_area).delete(delete_area))
        .route("/api/zones", post(add_zone))
        .route("/api/zones/{id}", put(update_zone).delete(delete_zone))
        .route("/api/solve/dc", post(solve_dc))
        .route("/api/solve/nr", post(solve_nr)) // NEW ROUTE FOR NEWTON-RAPHSON
        .route("/api/export", get(export_raw))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind port 3000");

    println!("Mantis running at http://localhost:3000");

    axum::serve(listener, app).await.expect("Server error");
}
