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

    // Configure keep-alive for Azure Container Apps (shorter interval)
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive")
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
                        warn!(record_id, "Notification received but record not found (race condition?)");
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
