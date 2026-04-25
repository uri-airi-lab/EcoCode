use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::{Stream, StreamExt};
use std::convert::Infallible;
use tokio::time::Duration;
use tokio_stream::wrappers::BroadcastStream;

use crate::state::AppState;

/// Upgrades an HTTP connection to a Server-Sent Events (SSE) stream.
/// Emits live updates across the application (e.g., new records or run state changes).
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    eprintln!("[SSE Handler] New client connected");
    
    let rx = state.tx.subscribe();
    let broadcast_stream = BroadcastStream::new(rx);

    // Create the main stream that filters and maps broadcast messages
    let stream = broadcast_stream.filter_map(|res| async move {
        match res {
            Ok(msg) => {
                eprintln!("[SSE Handler] Sending event to client");
                Some(Ok(Event::default().data(msg)))
            }
            Err(e) => {
                eprintln!("[SSE Handler] Stream error: {:?}", e);
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
    eprintln!("[SSE Watcher] Starting background watcher...");

    loop {
        match crate::db::records::fetch_latest_record_with_details(&state).await {
            Ok(Some(record)) if record.id > last_record_id => {
                eprintln!("[SSE Watcher] New record detected: id={}, run_id={}", record.id, record.run_id);
                if let Ok(payload) = serde_json::to_string(&record) {
                    match state.tx.send(payload.clone()) {
                        Ok(receiver_count) => {
                            eprintln!("[SSE Watcher] Broadcasted to {} receivers", receiver_count);
                        }
                        Err(_) => {
                            // Channel has no receivers - this is normal when no clients are connected
                            eprintln!("[SSE Watcher] No active receivers (no clients connected)");
                        }
                    }
                } else {
                    eprintln!("[SSE Watcher] Failed to serialize record {}", record.id);
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
                eprintln!("[SSE Watcher] Database error: {}", err.message);
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
