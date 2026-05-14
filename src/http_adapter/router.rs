use std::io;

use zero_engine::EngineHandle;

use super::handlers;

/// A parsed HTTP request ready for routing.
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Result of routing: either an immediate response body, or an SSE stream.
pub enum RouteResult {
    Respond(String, Vec<u8>),
    Sse {
        subscriber: zero_engine::EventSubscriber,
        catch_up: Vec<zero_api::RawApiEvent>,
    },
}

/// Route the request to the appropriate handler.
pub fn route(request: &HttpRequest, handle: &EngineHandle) -> RouteResult {
    let (method, path) = (request.method.as_str(), request.path.as_str());

    // Normalize away the /api/v1 prefix so we can match both forms.
    let api_path = path.strip_prefix("/api/v1").unwrap_or(path);
    let query = path_query_part(path);

    // Handle CORS preflight.
    if method == "OPTIONS" {
        return RouteResult::Respond(
            "HTTP/1.1 204 No Content\r\n".to_owned(),
            Vec::new(),
        );
    }

    match (method, api_path) {
        (m, p) if p.starts_with("/api/v1/") => match (m, p) {
            // ── /api/v1/* ──────────────────────────────────────────
            ("GET", "/api/v1/capabilities") => json_response(handlers::capabilities(handle)),
            ("GET", "/api/v1/health") => json_response(handlers::health(handle)),
            ("GET", "/api/v1/config") => json_response(handlers::config(handle)),
            ("GET", "/api/v1/runtime") => json_response(handlers::runtime(handle)),
            ("GET", "/api/v1/stats") => json_response(handlers::stats(handle)),
            ("GET", "/api/v1/flows") => json_response(handlers::flows_list(handle, query)),
            ("POST", "/api/v1/commands") => command_response(handlers::commands(handle, &request.body)),
            ("GET", "/api/v1/events/stream") => {
                match handlers::events_stream(handle, query, &request.headers) {
                    Ok((subscriber, catch_up)) => RouteResult::Sse { subscriber, catch_up },
                    Err((status, body)) => RouteResult::Respond(status.to_owned(), body),
                }
            }
            ("GET", "/api/v1/events") => json_response(handlers::events_snapshot(handle)),
            _ if path_segments(p) == 3 && p.starts_with("/api/v1/flows/") => {
                let flow_id = &p["/api/v1/flows/".len()..];
                json_response(handlers::flow_get(handle, flow_id))
            }
            ("GET", "/api/v1/policies") => json_response(handlers::policies_list(handle)),
            _ if path_segments(p) == 3 && p.starts_with("/api/v1/policies/") => {
                let policy_tag = &p["/api/v1/policies/".len()..];
                json_response(handlers::policy_get(handle, policy_tag))
            }
            _ => not_found_response(),
        },

        // ── compatibility endpoints ───────────────────────────────
        ("GET", "/status") => json_response(handlers::runtime(handle)),
        ("GET", "/config") => json_response(handlers::config(handle)),
        ("GET", "/runtime") => json_response(handlers::runtime(handle)),
        ("GET", "/events") => json_response(handlers::events_snapshot(handle)),
        ("POST", "/commands") => command_response(handlers::commands(handle, &request.body)),
        ("POST", p) if path_segments(p) == 3 && p.starts_with("/selectors/") => {
            match parse_selector_path(p) {
                Some((group, target)) => {
                    command_response(handlers::selector_update(handle, group, target))
                }
                None => not_found_response(),
            }
        }
        _ if method == "GET" => not_found_response(),
        _ => method_not_allowed_response(),
    }
}

fn json_response(result: io::Result<Vec<u8>>) -> RouteResult {
    match result {
        Ok(body) => RouteResult::Respond("HTTP/1.1 200 OK\r\n".to_owned(), body),
        Err(err) => {
            let body = format!(r#"{{"error":"{}"}}"#, err);
            RouteResult::Respond(
                "HTTP/1.1 500 Internal Server Error\r\n".to_owned(),
                body.into_bytes(),
            )
        }
    }
}

fn command_response(result: Result<Vec<u8>, (&'static str, Vec<u8>)>) -> RouteResult {
    match result {
        Ok(body) => RouteResult::Respond("HTTP/1.1 200 OK\r\n".to_owned(), body),
        Err((status, body)) => RouteResult::Respond(status.to_owned(), body),
    }
}

fn not_found_response() -> RouteResult {
    RouteResult::Respond(
        "HTTP/1.1 404 Not Found\r\n".to_owned(),
        br#"{"error":"not found"}"#.to_vec(),
    )
}

fn method_not_allowed_response() -> RouteResult {
    RouteResult::Respond(
        "HTTP/1.1 405 Method Not Allowed\r\n".to_owned(),
        br#"{"error":"method not allowed"}"#.to_vec(),
    )
}

fn path_segments(path: &str) -> usize {
    path.split('/').filter(|seg| !seg.is_empty()).count()
}

fn parse_selector_path(path: &str) -> Option<(&str, &str)> {
    let segments: Vec<&str> = path.split('/').collect();
    match segments.as_slice() {
        ["", "selectors", group_tag, outbound_tag]
            if !group_tag.is_empty() && !outbound_tag.is_empty() =>
        {
            Some((group_tag, outbound_tag))
        }
        _ => None,
    }
}

fn path_query_part(path: &str) -> &str {
    path.split_once('?').map(|(_, q)| q).unwrap_or("")
}
