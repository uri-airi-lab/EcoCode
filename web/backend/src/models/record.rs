use serde::Serialize;

/// Represents a raw record row from the database containing energy and usage metrics.
#[derive(Serialize, Clone, sqlx::FromRow)]
pub struct RecordPointRow {
    pub id: i64,
    pub run_id: i64,
    pub pid: i64,
    pub timestamp: String,
    pub cpu_usage: Option<f64>,
    pub cpu_energy: Option<f64>,
    pub gpu_usage: Option<f64>,
    pub gpu_energy: Option<f64>,
    pub mem_usage: Option<f64>,
    pub mem_energy: Option<f64>,
    pub igpu_usage: Option<f64>,
    pub igpu_energy: Option<f64>,
    // ML semantic fields from the Python SDK
    pub phase: Option<String>,
    pub tokens_processed: Option<i64>,
    pub training_step: Option<i64>,
    pub loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub sequence_length: Option<i32>,
    pub accuracy: Option<f64>,
}

/// Represents a complete record point with nested GPU and Socket specific metrics.
#[derive(Serialize, Clone)]
pub struct RecordPoint {
    pub id: i64,
    pub run_id: i64,
    pub pid: i64,
    pub timestamp: String,
    pub cpu_usage: Option<f64>,
    pub cpu_energy: Option<f64>,
    pub gpu_usage: Option<f64>,
    pub gpu_energy: Option<f64>,
    pub mem_usage: Option<f64>,
    pub mem_energy: Option<f64>,
    pub igpu_usage: Option<f64>,
    pub igpu_energy: Option<f64>,
    pub gpu_details: Vec<GpuRecordPoint>,
    pub socket_details: Vec<SocketRecordPoint>,
    // ML semantic fields from the Python SDK
    pub phase: Option<String>,
    pub tokens_processed: Option<i64>,
    pub training_step: Option<i64>,
    pub loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub sequence_length: Option<i32>,
    pub accuracy: Option<f64>,
}

/// Represents an individual GPU's metrics at a specific record point.
#[derive(Serialize, Clone, sqlx::FromRow)]
pub struct GpuRecordPoint {
    pub id: i64,
    pub record_id: i64,
    pub device_index: i32,
    pub gpu_usage: f64,
    pub gpu_energy: f64,
}

/// Represents an individual CPU socket domain's metrics at a specific record point.
#[derive(Serialize, Clone, sqlx::FromRow)]
pub struct SocketRecordPoint {
    pub id: i64,
    pub record_id: i64,
    pub socket: i32,
    pub domain: String,
    pub energy: f64,
}
