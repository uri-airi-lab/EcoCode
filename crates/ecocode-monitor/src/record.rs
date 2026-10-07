//! One sample: energy attributed to the process tree, markers and a timestamp.
//!
//! The serialised form is what the JSON exporter writes; the CSV exporter uses
//! [`Record::to_vec`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Per-GPU metrics for a single measurement record
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GpuMetric {
    pub device_index: u32,
    pub gpu_usage: f64,  // percentage (0-100)
    pub gpu_energy: f64, // Joules (Watt-seconds)
}

/// Per-socket metrics for a single measurement record (every non-GPU domain)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SocketMetric {
    pub socket: i16,
    pub domain: String, // "cpu", "dram", "igpu", "ane", or a sensor-specific name
    pub energy: f64,    // Joules (Watt-seconds)
}

/// Represents a single measurement record
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub id: u32,
    pub run_id: i64,
    pub pid: u32,
    pub timestamp: String, // RFC3339 UTC timestamp
    pub elapsed_secs: f64, // seconds since previous sample (used by exporters to compute Watts)
    pub cpu_usage: f64,    // percentage (0-100)
    pub cpu_energy: f64,   // Joules (Watt-seconds) (aggregate across sockets)
    pub gpu_usage: f64,    // percentage (0-100) (aggregate across GPUs)
    pub gpu_energy: f64,   // Joules (Watt-seconds) (aggregate across GPUs)
    pub mem_usage: f64,    // percentage (0-100), always available
    pub mem_energy: Option<f64>, // Joules (Watt-seconds), None when no DRAM domain on this system
    pub igpu_usage: Option<f64>, // None: no per-process iGPU sensor available
    pub igpu_energy: Option<f64>, // None when no iGPU domain on this system
    pub gpu_metrics: Vec<GpuMetric>,       // per-GPU breakdown
    pub socket_metrics: Vec<SocketMetric>, // per-socket breakdown
    // ML semantic fields, sent by the Python SDK as markers
    pub phase: Option<String>,
    pub tokens_processed: Option<i64>,
    pub training_step: Option<i64>,
    pub loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub sequence_length: Option<i32>,
    // Omitted when unset, so output is unchanged for runs that don't log them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_metrics: Option<HashMap<String, f64>>,
}

impl Record {
    pub fn to_vec(&self) -> Vec<String> {
        // Divide Joule values by elapsed_secs to produce average Watts per interval.
        let es = self.elapsed_secs.max(0.001);
        let mut v = vec![
            self.id.to_string(),
            self.run_id.to_string(),
            self.pid.to_string(),
            self.timestamp.clone(),
            self.cpu_usage.to_string(),
            (self.cpu_energy / es).to_string(),
            self.gpu_usage.to_string(),
            (self.gpu_energy / es).to_string(),
            self.mem_usage.to_string(),
            self.mem_energy
                .map_or_else(|| "N/A".to_string(), |j| (j / es).to_string()),
            self.igpu_usage
                .map_or_else(|| "N/A".to_string(), |v| v.to_string()),
            self.igpu_energy
                .map_or_else(|| "N/A".to_string(), |j| (j / es).to_string()),
        ];
        for gm in &self.gpu_metrics {
            v.push(gm.gpu_usage.to_string());
            v.push((gm.gpu_energy / es).to_string());
        }
        v.push(self.phase.clone().unwrap_or_else(|| "N/A".to_string()));
        v.push(
            self.tokens_processed
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v.push(
            self.training_step
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v.push(
            self.loss
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v.push(
            self.learning_rate
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v.push(
            self.sequence_length
                .map_or_else(|| "N/A".to_string(), |n| n.to_string()),
        );
        v
    }
}

#[cfg(test)]
mod tests {
    //! Golden tests: expected strings were captured from the original
    //! implementation of `Record` serialisation (JSON exporter) and `to_vec`
    //! (CSV exporter) and must not drift.
    use super::*;

    fn full() -> Record {
        Record {
            id: 7,
            run_id: 42,
            pid: 1234,
            timestamp: "2026-04-28T14:34:47.292762899+00:00".into(),
            elapsed_secs: 0.5,
            cpu_usage: 8.609794616699219,
            cpu_energy: 0.05046510577453613,
            gpu_usage: 42.0,
            gpu_energy: 10.854075,
            mem_usage: 7.99,
            mem_energy: Some(0.0123),
            igpu_usage: None,
            igpu_energy: Some(0.001),
            gpu_metrics: vec![
                GpuMetric { device_index: 0, gpu_usage: 42.0, gpu_energy: 10.854075 },
                GpuMetric { device_index: 1, gpu_usage: 3.0, gpu_energy: 0.25 },
            ],
            socket_metrics: vec![
                SocketMetric { socket: 0, domain: "cpu".into(), energy: 0.05046510577453613 },
                SocketMetric { socket: 0, domain: "dram".into(), energy: 0.0123 },
            ],
            phase: Some("forward".into()),
            tokens_processed: Some(4096),
            training_step: Some(12),
            loss: Some(1.4567),
            learning_rate: Some(3e-4),
            sequence_length: Some(512),
            accuracy: None,
            custom_metrics: None,
        }
    }

    fn bare() -> Record {
        Record {
            id: 1,
            run_id: 0,
            pid: 99,
            timestamp: "t".into(),
            elapsed_secs: 1.0,
            cpu_usage: 0.0,
            cpu_energy: 0.0,
            gpu_usage: 0.0,
            gpu_energy: 0.0,
            mem_usage: 0.0,
            mem_energy: None,
            igpu_usage: None,
            igpu_energy: None,
            gpu_metrics: vec![],
            socket_metrics: vec![],
            phase: None,
            tokens_processed: None,
            training_step: None,
            loss: None,
            learning_rate: None,
            sequence_length: None,
            accuracy: None,
            custom_metrics: None,
        }
    }

    #[test]
    fn json_record_format_is_unchanged() {
        assert_eq!(
            serde_json::to_string(&full()).unwrap(),
            r#"{"id":7,"run_id":42,"pid":1234,"timestamp":"2026-04-28T14:34:47.292762899+00:00","elapsed_secs":0.5,"cpu_usage":8.609794616699219,"cpu_energy":0.05046510577453613,"gpu_usage":42.0,"gpu_energy":10.854075,"mem_usage":7.99,"mem_energy":0.0123,"igpu_usage":null,"igpu_energy":0.001,"gpu_metrics":[{"device_index":0,"gpu_usage":42.0,"gpu_energy":10.854075},{"device_index":1,"gpu_usage":3.0,"gpu_energy":0.25}],"socket_metrics":[{"socket":0,"domain":"cpu","energy":0.05046510577453613},{"socket":0,"domain":"dram","energy":0.0123}],"phase":"forward","tokens_processed":4096,"training_step":12,"loss":1.4567,"learning_rate":0.0003,"sequence_length":512}"#
        );
        assert_eq!(
            serde_json::to_string(&bare()).unwrap(),
            r#"{"id":1,"run_id":0,"pid":99,"timestamp":"t","elapsed_secs":1.0,"cpu_usage":0.0,"cpu_energy":0.0,"gpu_usage":0.0,"gpu_energy":0.0,"mem_usage":0.0,"mem_energy":null,"igpu_usage":null,"igpu_energy":null,"gpu_metrics":[],"socket_metrics":[],"phase":null,"tokens_processed":null,"training_step":null,"loss":null,"learning_rate":null,"sequence_length":null}"#
        );
    }

    #[test]
    fn accuracy_and_custom_metrics_appear_only_when_logged() {
        let mut r = bare();
        r.accuracy = Some(0.9);
        r.custom_metrics = Some(HashMap::from([("grad_norm".to_string(), 0.75)]));
        assert!(serde_json::to_string(&r)
            .unwrap()
            .ends_with(r#""sequence_length":null,"accuracy":0.9,"custom_metrics":{"grad_norm":0.75}}"#));
    }

    #[test]
    fn csv_row_format_is_unchanged() {
        assert_eq!(
            full().to_vec(),
            [
                "7", "42", "1234", "2026-04-28T14:34:47.292762899+00:00", "8.609794616699219",
                "0.10093021154907227", "42", "21.70815", "7.99", "0.0246", "N/A", "0.002", "42",
                "21.70815", "3", "0.5", "forward", "4096", "12", "1.4567", "0.0003", "512",
            ]
        );
        assert_eq!(
            bare().to_vec(),
            ["1", "0", "99", "t", "0", "0", "0", "0", "0", "N/A", "N/A", "N/A", "N/A", "N/A", "N/A", "N/A", "N/A", "N/A"]
        );
    }
}
