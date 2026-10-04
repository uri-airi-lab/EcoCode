//! Semantic marker messages sent by the Python SDK.
//!
//! The SDK writes newline-delimited JSON, either to an inherited pipe (attached
//! mode, `ecocode run`) or to the monitor's stdin (standalone mode). Each line
//! is one event, e.g. `{"event": "phase", "value": "forward"}`. The sampling
//! loop reads the latest [`MarkerState`] on every tick and attaches it to the
//! record.

use std::collections::HashMap;

use serde_json::Value;

/// Latest marker values received from the SDK.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MarkerState {
    pub current_phase: Option<String>,
    pub tokens_processed: Option<i64>,
    pub training_step: Option<i64>,
    pub loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub sequence_length: Option<i32>,
    pub accuracy: Option<f64>,
    pub custom_metrics: HashMap<String, f64>,
}

/// Parses one line of the marker stream. Blank lines and invalid JSON yield `None`.
pub fn parse_line(line: &str) -> Option<Value> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(line).ok()
}

impl MarkerState {
    /// Updates the state from one marker event. Unknown events are ignored.
    pub fn apply(&mut self, event: &Value) {
        match event.get("event").and_then(|e| e.as_str()) {
            Some("phase") => {
                self.current_phase = event["value"].as_str().map(|s| s.to_string());
            }
            Some("epoch") => {
                let n = event["value"].as_i64().unwrap_or(0);
                self.current_phase = Some(format!("epoch:{}", n));
            }
            Some("tokens") => {
                self.tokens_processed = event["count"].as_i64();
            }
            Some("step") => {
                self.training_step = event["value"].as_i64();
            }
            Some("loss") => {
                self.loss = event["value"].as_f64();
            }
            Some("learning_rate") => {
                self.learning_rate = event["value"].as_f64();
            }
            Some("sequence_length") => {
                self.sequence_length = event["value"].as_i64().map(|n| n as i32);
            }
            Some("accuracy") => {
                self.accuracy = event["value"].as_f64();
            }
            Some("custom") => {
                if let (Some(key), Some(value)) = (event["key"].as_str(), event["value"].as_f64()) {
                    self.custom_metrics.insert(key.to_string(), value);
                }
            }
            _ => {}
        }
    }

    /// Custom metrics, or `None` when no custom metric has been logged.
    pub fn custom_metrics_opt(&self) -> Option<HashMap<String, f64>> {
        if self.custom_metrics.is_empty() {
            None
        } else {
            Some(self.custom_metrics.clone())
        }
    }
}
