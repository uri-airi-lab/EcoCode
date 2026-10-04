//! Intel Mac CPU package power from the System Management Controller (SMC).
//!
//! Intel Macs have no IOReport Energy Model. The SMC exposes the CPU package
//! power in watts (key `PCPT` or `PCPC` depending on the model), readable
//! without root through the `AppleSMC` IOKit user client. It is an
//! instantaneous power reading, so energy is integrated between samples
//! (trapezoid rule) and reported as [`Quality::Estimated`].

use std::ffi::c_void;
use std::time::Instant;

use super::iokit::{self, Object};
use crate::domain::{Channel, Domain, Quality, Reading};
use crate::{EnergySensor, SensorError};

/// Candidate CPU package power keys, most specific first.
pub const CPU_POWER_KEYS: [&str; 2] = ["PCPT", "PCPC"];

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KeyDataVers {
    major: u8,
    minor: u8,
    build: u8,
    reserved: u8,
    release: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PLimitData {
    version: u16,
    length: u16,
    cpu_p_limit: u32,
    gpu_p_limit: u32,
    mem_p_limit: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KeyInfo {
    data_size: u32,
    data_type: u32,
    data_attributes: u8,
}

/// `SMCKeyData_t`, the in/out buffer of the AppleSMC user client.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KeyData {
    key: u32,
    vers: KeyDataVers,
    p_limit_data: PLimitData,
    key_info: KeyInfo,
    result: u8,
    status: u8,
    data8: u8,
    data32: u32,
    bytes: [u8; 32],
}

const _: () = assert!(std::mem::size_of::<KeyData>() == 80);

const HANDLE_YPC_EVENT: u32 = 2;
const CMD_READ_KEY: u8 = 5;
const CMD_GET_KEY_INFO: u8 = 9;

fn fourcc(s: &str) -> u32 {
    let b = s.as_bytes();
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// A connection to the AppleSMC user client.
pub struct Smc {
    connection: u32,
}

impl Smc {
    pub fn open() -> Option<Self> {
        unsafe {
            let matching = iokit::IOServiceMatching(c"AppleSMC".as_ptr());
            let service = Object(iokit::IOServiceGetMatchingService(iokit::MAIN_PORT_DEFAULT, matching));
            if service.0 == 0 {
                return None;
            }
            let mut connection = 0;
            (iokit::IOServiceOpen(service.0, iokit::mach_task_self(), 0, &mut connection) == iokit::KERN_SUCCESS)
                .then_some(Smc { connection })
        }
    }

    fn call(&self, input: &KeyData) -> Option<KeyData> {
        let mut output = KeyData::default();
        let mut size = std::mem::size_of::<KeyData>();
        let rc = unsafe {
            iokit::IOConnectCallStructMethod(
                self.connection,
                HANDLE_YPC_EVENT,
                input as *const KeyData as *const c_void,
                std::mem::size_of::<KeyData>(),
                &mut output as *mut KeyData as *mut c_void,
                &mut size,
            )
        };
        (rc == iokit::KERN_SUCCESS && output.result == 0).then_some(output)
    }

    /// Reads a key and decodes it as a number.
    pub fn read(&self, key: &str) -> Option<f64> {
        let info = self.call(&KeyData { key: fourcc(key), data8: CMD_GET_KEY_INFO, ..Default::default() })?;
        let size = info.key_info.data_size as usize;
        if size == 0 || size > 32 {
            return None;
        }
        let data = self.call(&KeyData {
            key: fourcc(key),
            key_info: info.key_info,
            data8: CMD_READ_KEY,
            ..Default::default()
        })?;
        decode(&info.key_info.data_type.to_be_bytes(), &data.bytes[..size])
    }
}

impl Drop for Smc {
    fn drop(&mut self) {
        unsafe { iokit::IOServiceClose(self.connection) };
    }
}

/// Decodes an SMC value by its four-character type. Fixed-point and integer
/// types are big-endian; `flt ` is a native (little-endian) f32.
pub fn decode(kind: &[u8; 4], bytes: &[u8]) -> Option<f64> {
    let be16 = || Some(u16::from_be_bytes(bytes.get(..2)?.try_into().ok()?));
    let signed_fixed = |frac_bits: u32| be16().map(|v| v as i16 as f64 / (1u32 << frac_bits) as f64);
    match kind {
        b"flt " => Some(f32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as f64),
        b"ui8 " => bytes.first().map(|&v| v as f64),
        b"ui16" => be16().map(|v| v as f64),
        b"ui32" => Some(u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?) as f64),
        b"fpe2" => be16().map(|v| v as f64 / 4.0),
        b"fp88" => be16().map(|v| v as f64 / 256.0),
        b"sp78" => signed_fixed(8),
        b"sp87" => signed_fixed(7),
        b"sp96" => signed_fixed(6),
        b"spa5" => signed_fixed(5),
        _ => None,
    }
}

/// CPU package energy integrated from SMC power readings.
pub struct SmcSensor {
    smc: Smc,
    key: &'static str,
    channels: Vec<Channel>,
    prev_watts: f64,
    prev_at: Instant,
}

impl SmcSensor {
    /// The first CPU power key that gives a plausible reading.
    pub fn open() -> Option<Self> {
        let smc = Smc::open()?;
        let (key, watts) = CPU_POWER_KEYS
            .iter()
            .find_map(|&k| smc.read(k).filter(|w| w.is_finite() && (0.0..1000.0).contains(w)).map(|w| (k, w)))?;
        Some(SmcSensor {
            smc,
            key,
            channels: vec![Channel {
                domain: Domain::Cpu,
                socket: 0,
                name: format!("CPU package (SMC {key})"),
                quality: Quality::Estimated,
            }],
            prev_watts: watts,
            prev_at: Instant::now(),
        })
    }

    pub fn key(&self) -> &str {
        self.key
    }
}

impl EnergySensor for SmcSensor {
    fn name(&self) -> &'static str {
        "smc"
    }

    fn channels(&self) -> &[Channel] {
        &self.channels
    }

    fn read(&mut self, _tree: &[u32]) -> Result<Vec<Reading>, SensorError> {
        let watts = self
            .smc
            .read(self.key)
            .filter(|w| w.is_finite() && *w >= 0.0)
            .ok_or_else(|| SensorError::new("smc", format!("could not read key {}", self.key)))?;
        let now = Instant::now();
        let secs = now.duration_since(self.prev_at).as_secs_f64();
        let joules = (self.prev_watts + watts) / 2.0 * secs;
        self.prev_watts = watts;
        self.prev_at = now;
        Ok(vec![Reading::energy(joules)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_smc_types() {
        assert_eq!(decode(b"flt ", &12.5f32.to_le_bytes()), Some(12.5));
        assert_eq!(decode(b"sp96", &[0x03, 0x20]), Some(12.5)); // 800 / 64
        assert_eq!(decode(b"sp78", &[0xff, 0x80]), Some(-0.5));
        assert_eq!(decode(b"fpe2", &[0x00, 0x32]), Some(12.5)); // 50 / 4
        assert_eq!(decode(b"ui32", &[0, 0, 4, 0]), Some(1024.0));
        assert_eq!(decode(b"ui8 ", &[7]), Some(7.0));
        assert_eq!(decode(b"ch8*", &[1, 2]), None);
        assert_eq!(decode(b"ui32", &[1]), None);
    }

    #[test]
    fn fourcc_is_big_endian() {
        assert_eq!(fourcc("#KEY"), 0x234B_4559);
    }

    #[test]
    #[ignore = "needs a real Mac, not a VM (run with --ignored)"]
    fn reads_the_smc_key_count() {
        let smc = Smc::open().expect("AppleSMC not available");
        let keys = smc.read("#KEY").expect("could not read #KEY");
        assert!(keys > 50.0, "#KEY = {keys}");
        // Power keys: PCPT/PCPC exist on Intel Macs, PSTR (system total) on Apple Silicon.
        for key in CPU_POWER_KEYS.iter().chain(&["PSTR"]) {
            println!("SMC {key} = {:?} W", smc.read(key));
        }
    }
}
