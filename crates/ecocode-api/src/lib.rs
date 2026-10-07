//! Client for the EcoCode web backend, identical on every OS.
//!
//! - [`types`]: the records and system info sent to the backend
//! - [`client`]: blocking HTTP client for the backend endpoints

pub mod client;
pub mod types;

pub use client::{BASE_URL, EcoCodeHttpClient};
