use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{Stream, StreamExt};
use std::convert::Infallible;
use tokio::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, error, info, warn};

use crate::state::AppState;

/// Upgrades an HTTP connection to a Server-Sent Events (SSE) stream.
/// Emits live updates across the application (e.g., new records or run state changes).
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("SSE client connected");

    let rx = state.tx.subscribe();
    let broadcast_stream = BroadcastStream::new(rx);

    // Create the main stream that filters and maps broadcast messages
    let stream = broadcast_stream.filter_map(|res| async move {
        match res {
            Ok(msg) => {
                debug!("Sending SSE event to client");
                Some(Ok(Event::default().data(msg)))
            }
            Err(e) => {
                warn!(error = ?e, "SSE stream error");
                None
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Background task that continuously polls the database for new records.
/// If a new record is found, it serializes and broadcasts it over SSE.
pub async fn watch_changes(state: AppState) {
    let mut last_record_id = 0_i64;
    info!("SSE background watcher started");

    loop {
        match crate::db::records::fetch_latest_record_with_details(&state).await {
            Ok(Some(record)) if record.id > last_record_id => {
                debug!(record_id = record.id, run_id = record.run_id, "New record detected, broadcasting");
                if let Ok(payload) = serde_json::to_string(&record) {
                    match state.tx.send(payload.clone()) {
                        Ok(receiver_count) => {
                            debug!(record_id = record.id, receiver_count, "SSE event broadcast");
                        }
                        Err(_) => {
                            // Channel has no receivers - this is normal when no clients are connected
                            debug!(record_id = record.id, "No active SSE receivers");
                        }
                    }
                } else {
                    error!(record_id = record.id, "Failed to serialize record for SSE broadcast");
                }
                last_record_id = record.id;
            }
            Ok(Some(record)) => {
                last_record_id = last_record_id.max(record.id);
            }
            Ok(None) => {
                // No records yet
            }
            Err(err) => {
                error!(error = %err.message, "SSE watcher database error");
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
