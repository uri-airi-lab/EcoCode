//! Fake sensors for tests and demos on any OS (`--sensors fake`).
//!
//! Each channel draws constant power, so energy grows with wall-clock time.
//! The fake GPU reports utilisation samples for the monitored process and for
//! one other process at equal load, so half of the GPU energy is attributed to
//! the process tree.

use std::time::Instant;

use crate::domain::{Channel, Domain, GpuUtilSample, Quality, Reading};
use crate::{EnergySensor, SensorError};

/// Constant power per channel, in watts: cpu, dram, gpu0.
pub const FAKE_WATTS: [f64; 3] = [15.0, 3.0, 60.0];
/// SM utilisation the fake GPU reports for each process.
pub const FAKE_GPU_UTIL: u32 = 40;
/// PID of the fake GPU's other (unmonitored) process.
pub const FAKE_OTHER_PID: u32 = u32::MAX - 7;

pub struct FakeSensor {
    channels: Vec<Channel>,
    last: Instant,
    timestamp: u64,
}

impl FakeSensor {
    pub fn new() -> Self {
        let channel = |domain, name: &str| Channel {
            domain,
            socket: 0,
            name: name.into(),
            quality: Quality::Estimated,
        };
        FakeSensor {
            channels: vec![
                channel(Domain::Cpu, "fake cpu"),
                channel(Domain::Dram, "fake dram"),
                channel(Domain::Gpu(0), "Fake GPU"),
            ],
            last: Instant::now(),
            timestamp: 0,
        }
    }
}

impl Default for FakeSensor {
    fn default() -> Self {
        Self::new()
    }
}

impl EnergySensor for FakeSensor {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn channels(&self) -> &[Channel] {
        &self.channels
    }

    fn read(&mut self, tree: &[u32]) -> Result<Vec<Reading>, SensorError> {
        let now = Instant::now();
        let secs = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        self.timestamp += 1;

        let sample = |pid| GpuUtilSample {
            pid,
            timestamp: self.timestamp,
            sm_util: FAKE_GPU_UTIL,
            mem_util: 10,
            enc_util: 0,
            dec_util: 0,
        };
        let mut gpu_samples: Vec<GpuUtilSample> = tree.first().map(|&p| sample(p)).into_iter().collect();
        gpu_samples.push(sample(FAKE_OTHER_PID));

        Ok(vec![
            Reading::energy(FAKE_WATTS[0] * secs),
            Reading::energy(FAKE_WATTS[1] * secs),
            Reading { joules: FAKE_WATTS[2] * secs, util_samples: Some(gpu_samples), tree_share: None },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_is_power_times_time() {
        let mut s = FakeSensor::new();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let r = s.read(&[42]).unwrap();
        assert_eq!(r.len(), s.channels().len());
        let secs = r[0].joules / FAKE_WATTS[0];
        assert!((0.09..0.5).contains(&secs), "elapsed {secs}");
        assert!((r[2].joules / FAKE_WATTS[2] - secs).abs() < 1e-9);
        let samples = r[2].util_samples.as_ref().unwrap();
        assert_eq!(samples.iter().map(|s| s.pid).collect::<Vec<_>>(), [42, FAKE_OTHER_PID]);
    }
}
