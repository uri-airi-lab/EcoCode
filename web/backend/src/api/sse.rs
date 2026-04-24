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
    let rx = state.tx.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(msg) => Some(Ok(Event::default().data(msg))),
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Background task that continuously polls the database for new records.
/// If a new record is found, it serializes and broadcasts it over SSE.
pub async fn watch_changes(state: AppState) {
    let mut last_record_id = 0_i64;

    loop {
        match crate::db::records::fetch_latest_record_with_details(&state).await {
            Ok(Some(record)) if record.id > last_record_id => {
                if let Ok(payload) = serde_json::to_string(&record) {
                    let _ = state.tx.send(payload);
                }
                last_record_id = record.id;
            }
            Ok(Some(record)) => {
                last_record_id = last_record_id.max(record.id);
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!("watcher error: {}", err.message);
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
