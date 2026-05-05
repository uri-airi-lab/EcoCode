//! Shared sensor and HTTP client library for the EcoCode CLI and Python SDK.
//!
//! This crate provides:
//! - **RAPL sensor** — reads CPU/DRAM/iGPU energy from `/sys/class/powercap/intel-rapl/`
//! - **GPU sensor** — reads NVIDIA GPU energy and per-process utilisation via NVML
//! - **HTTP client** — blocking HTTP client for the EcoCode web backend (4 endpoints)

pub mod http_client;
pub mod sensor;
