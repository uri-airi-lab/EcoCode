//! Server-Sent Events (SSE) — real-time record streaming to browser clients.
//!
//! Architecture:
//! - A dedicated background task (`watch_changes`) opens a persistent PostgreSQL `LISTEN`
//!   connection on the `record_inserted` channel.
//! - When the CLI ingests a new record via `POST /api/ingest/run/{id}/record`, the handler
//!   immediately broadcasts the record JSON to an in-memory `tokio::sync::broadcast` channel.
//! - `sse_handler` subscribes each browser client to that broadcast channel and streams
//!   each message as an SSE `data:` event.
//! - Keep-alive pings are sent every 15 seconds to prevent proxy timeouts (required for
//!   Azure Container Apps and similar managed hosting environments).
//!
//! The `record_inserted` PostgreSQL NOTIFY channel is a fallback for cases where the
//! direct broadcast (written in `ingest_record`) does not reach clients — e.g. when
//! multiple backend replicas are running and a record is written on a different instance.

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{Stream, StreamExt};
use serde::Deserialize;
use std::convert::Infallible;
use tokio::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, error, info, warn};

use crate::state::AppState;

/// Optional query parameters for the SSE endpoint.
#[derive(Deserialize, Default)]
pub struct SseQuery {
    /// When set, only records belonging to this run are forwarded.
    pub run_id: Option<i64>,
}

/// Upgrades an HTTP connection to a Server-Sent Events (SSE) stream.
/// Emits live updates across the application (e.g., new records or run state changes).
/// Accepts an optional `?run_id=N` query parameter to filter events server-side.
pub async fn sse_handler(
    State(state): State<AppState>,
    Query(query): Query<SseQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!(run_id = ?query.run_id, "SSE client connected");

    let rx = state.tx.subscribe();
    let broadcast_stream = BroadcastStream::new(rx);
    let filter_run_id = query.run_id;

    // Create the main stream that filters and maps broadcast messages.
    // When the broadcast channel lags (client too slow), send a "refresh" event
    // so the frontend knows to re-fetch rather than silently missing data.
    let stream = broadcast_stream.filter_map(move |res| async move {
        match res {
            Ok(msg) => {
                // If the client requested a specific run_id, filter out unrelated events.
                if let Some(rid) = filter_run_id {
                    // "run_updated:123" format
                    if msg.starts_with("run_updated:") {
                        let id_str = &msg["run_updated:".len()..];
                        if id_str.parse::<i64>().ok() != Some(rid) {
                            return None;
                        }
                    } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                        if val.get("run_id").and_then(|v| v.as_i64()) != Some(rid) {
                            return None;
                        }
                    }
                }
                debug!("Sending SSE event to client");
                Some(Ok(Event::default().data(msg)))
            }
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(count)) => {
                warn!(skipped = count, "SSE client lagged, sending refresh hint");
                Some(Ok(Event::default().event("refresh").data(format!("{{\"skipped\":{}}}", count))))
            }
        }
    });

    // Configure keep-alive for Azure Container Apps (shorter interval)
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

pub async fn watch_changes(state: AppState) {
    use sqlx::postgres::PgListener;

    let mut listener = match PgListener::connect(&state.db_url).await {
        Ok(l) => l,
        Err(e) => {
            error!(error = ?e, "Failed to connect dedicated PgListener for SSE");
            return;
        }
    };

    if let Err(e) = listener.listen("record_inserted").await {
        error!(error = ?e, "Failed to listen on 'record_inserted' channel");
        return;
    }

    info!("SSE background watcher started (Dedicated LISTEN/NOTIFY mode)");

    loop {
        match listener.recv().await {
            Ok(notification) => {
                let payload_str = notification.payload();
                let record_id: i64 = match payload_str.parse() {
                    Ok(id) => id,
                    Err(_) => {
                        warn!(payload = %payload_str, "Received invalid payload on 'record_inserted' channel");
                        continue;
                    }
                };

                debug!(record_id, "Notification received, fetching details");

                match crate::db::records::fetch_record_by_id_with_details(record_id, &state).await {
                    Ok(Some(record)) => {
                        if let Ok(json_payload) = serde_json::to_string(&record) {
                            let _ = state.tx.send(json_payload);
                            debug!(record_id, "SSE event broadcast via NOTIFY");
                        }
                    }
                    Ok(None) => {
                        warn!(
                            record_id,
                            "Notification received but record not found (race condition?)"
                        );
                    }
                    Err(e) => {
                        error!(record_id, error = ?e, "Failed to fetch record details for SSE");
                    }
                }
            }
            Err(e) => {
                error!(error = ?e, "PgListener connection lost, attempting to reconnect in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;

                // Attempt to reconnect
                match PgListener::connect(&state.db_url).await {
                    Ok(mut l) => {
                        if let Ok(_) = l.listen("record_inserted").await {
                            info!("PgListener reconnected successfully");
                            listener = l;
                        }
                    }
                    Err(_) => {
                        warn!("Reconnect attempt failed, will retry in next loop");
                    }
                }
            }
        }
    }
}
