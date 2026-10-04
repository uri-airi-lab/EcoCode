//! Golden test: the JSON posted to `/api/ingest/run/{id}/record` must stay
//! byte-identical to what the pre-restructure client sent.

use std::collections::HashMap;

use ecocode_api::types::{IngestRecord, SocketMetric};

#[test]
fn record_wire_format_is_unchanged() {
    let mut custom = HashMap::new();
    custom.insert("grad_norm".to_string(), 0.75);
    let record = IngestRecord {
        pid: 1234,
        timestamp: "2026-04-28T14:34:47Z".into(),
        elapsed_secs: Some(1.0),
        cpu_usage: 12.5,
        cpu_energy: 3.25,
        gpu_usage: 0.0,
        gpu_energy: 0.0,
        mem_usage: 1.5,
        mem_energy: None,
        igpu_usage: None,
        igpu_energy: Some(0.5),
        gpu_metrics: vec![],
        socket_metrics: vec![SocketMetric { socket: 0, domain: "cpu".into(), energy: 3.25 }],
        phase: Some("eval".into()),
        epoch_num: None,
        tokens_processed: Some(10),
        training_step: Some(2),
        loss: Some(0.5),
        learning_rate: None,
        sequence_length: None,
        accuracy: Some(0.9),
        custom_metrics: Some(custom),
    };
    assert_eq!(
        serde_json::to_string(&record).unwrap(),
        r#"{"pid":1234,"timestamp":"2026-04-28T14:34:47Z","elapsed_secs":1.0,"cpu_usage":12.5,"cpu_energy":3.25,"gpu_usage":0.0,"gpu_energy":0.0,"mem_usage":1.5,"mem_energy":null,"igpu_usage":null,"igpu_energy":0.5,"gpu_metrics":[],"socket_metrics":[{"socket":0,"domain":"cpu","energy":3.25}],"phase":"eval","epoch_num":null,"tokens_processed":10,"training_step":2,"loss":0.5,"learning_rate":null,"sequence_length":null,"accuracy":0.9,"custom_metrics":{"grad_norm":0.75}}"#
    );
}

#[test]
fn default_backend_is_fixed() {
    assert_eq!(ecocode_api::BASE_URL, "https://ecocode.harmonyhpc.io");
}
