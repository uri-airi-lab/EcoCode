//! Errors raised while reading sensors.

use std::fmt;

#[derive(Debug)]
pub struct SensorError {
    pub sensor: &'static str,
    pub message: String,
}

impl SensorError {
    pub fn new(sensor: &'static str, message: impl Into<String>) -> Self {
        SensorError { sensor, message: message.into() }
    }
}

impl fmt::Display for SensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.sensor, self.message)
    }
}

impl std::error::Error for SensorError {}
