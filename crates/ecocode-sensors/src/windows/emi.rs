//! Windows Energy Meter Interface (EMI).
//!
//! Windows exposes hardware energy counters through EMI devices (interface
//! `GUID_DEVICE_ENERGY_METER`), without a custom driver. On Intel systems the
//! channels are RAPL domains, e.g. `RAPL_Package0_PKG`, `RAPL_Package0_PP0`
//! (cores), `RAPL_Package0_PP1` (graphics), `RAPL_Package0_DRAM`. Many desktops
//! and AMD systems have no EMI device; then CPU energy is unavailable.
//!
//! Each channel reports cumulative energy in picowatt-hours (`IOCTL_EMI_GET_MEASUREMENT`).
//! Channels are mapped like on Linux: CPU = cores (PP0), falling back to the
//! package when no core channel exists; PP1 → integrated GPU; DRAM → memory.
//!
//! The byte formats below follow `emi.h` and are parsed without Windows APIs,
//! so they are tested on every OS.

use crate::domain::Domain;

/// `CTL_CODE(FILE_DEVICE_UNKNOWN, n, METHOD_BUFFERED, FILE_READ_ACCESS)`
const fn ioctl(function: u32) -> u32 {
    (0x22 << 16) | (1 << 14) | (function << 2)
}
pub const IOCTL_EMI_GET_VERSION: u32 = ioctl(0);
pub const IOCTL_EMI_GET_METADATA_SIZE: u32 = ioctl(1);
pub const IOCTL_EMI_GET_METADATA: u32 = ioctl(2);
pub const IOCTL_EMI_GET_MEASUREMENT: u32 = ioctl(3);

/// `EMI_MEASUREMENT_UNIT_PICOWATTHOURS`, the only unit EMI defines.
pub const UNIT_PICOWATT_HOURS: u32 = 0;

/// Picowatt-hours to joules (1 pWh = 3600 pJ).
pub fn pwh_to_joules(pwh: f64) -> f64 {
    pwh * 3600.0 / 1e12
}

/// One channel from an EMI device's metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct EmiChannel {
    pub name: String,
    pub unit: u32,
}

fn u16_at(buf: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(buf.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(buf.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(buf: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(buf.get(at..at + 8)?.try_into().ok()?))
}

/// UTF-16LE string of `bytes` bytes at `at`, up to the first NUL.
fn wstr_at(buf: &[u8], at: usize, bytes: usize) -> Option<String> {
    let raw = buf.get(at..at + bytes)?;
    let units: Vec<u16> = raw.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&u| u != 0).collect();
    Some(String::from_utf16_lossy(&units))
}

/// `EMI_METADATA_V1`: a single channel named after the metered hardware.
///
/// ```text
/// u32 MeasurementUnit | WCHAR HardwareOEM[16] | WCHAR HardwareModel[16]
/// u16 HardwareRevision | u16 MeteredHardwareNameSize | WCHAR MeteredHardwareName[]
/// ```
pub fn parse_metadata_v1(buf: &[u8]) -> Option<Vec<EmiChannel>> {
    let unit = u32_at(buf, 0)?;
    let name_size = u16_at(buf, 70)? as usize;
    Some(vec![EmiChannel { name: wstr_at(buf, 72, name_size)?, unit }])
}

/// `EMI_METADATA_V2`: several variable-size channels.
///
/// ```text
/// WCHAR HardwareOEM[16] | WCHAR HardwareModel[16] | u16 HardwareRevision | u16 ChannelCount
/// then per channel: u32 MeasurementUnit | u16 ChannelNameSize | WCHAR ChannelName[]
/// (the next channel starts right after the name: EMI_CHANNEL_V2_NEXT_CHANNEL)
/// ```
pub fn parse_metadata_v2(buf: &[u8]) -> Option<Vec<EmiChannel>> {
    let count = u16_at(buf, 66)? as usize;
    let mut at = 68;
    let mut channels = Vec::with_capacity(count);
    for _ in 0..count {
        let unit = u32_at(buf, at)?;
        let name_size = u16_at(buf, at + 4)? as usize;
        channels.push(EmiChannel { name: wstr_at(buf, at + 6, name_size)?, unit });
        at += 6 + name_size;
    }
    Some(channels)
}

/// Cumulative energy (pWh) per channel from `EMI_MEASUREMENT_DATA_V1/V2`:
/// an array of `{ u64 AbsoluteEnergy; u64 AbsoluteTime }`.
pub fn parse_measurements(buf: &[u8], channels: usize) -> Option<Vec<u64>> {
    (0..channels).map(|i| u64_at(buf, i * 16)).collect()
}

/// What an EMI channel measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Cores,
    Package,
    Graphics,
    Dram,
}

/// Classifies a channel by name (RAPL-style names and common alternatives).
pub fn classify(name: &str) -> Option<Role> {
    let n = name.to_ascii_uppercase();
    if n.contains("PSYS") || n.contains("PLATFORM") {
        None
    } else if n.contains("PP0") || n.contains("CORE") {
        Some(Role::Cores)
    } else if n.contains("PP1") || n.contains("GFX") || n.contains("GRAPHICS") {
        Some(Role::Graphics)
    } else if n.contains("DRAM") || n.contains("MEMORY") {
        Some(Role::Dram)
    } else if n.contains("PKG") || n.contains("PACKAGE") || n.contains("CPU") || n.contains("SOC") {
        Some(Role::Package)
    } else {
        None
    }
}

/// Socket number in names like `RAPL_Package1_PP0`, else 0.
pub fn socket_of(name: &str) -> i16 {
    let n = name.to_ascii_uppercase();
    n.find("PACKAGE")
        .map(|i| &n[i + "PACKAGE".len()..])
        .and_then(|rest| rest.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok())
        .unwrap_or(0)
}

/// Picks the channels to report: (index into `channels`, domain). CPU is the
/// core channel per socket, or the package channel when a socket has no core
/// channel, never both.
pub fn select(channels: &[EmiChannel]) -> Vec<(usize, Domain)> {
    let roles: Vec<Option<Role>> = channels
        .iter()
        .map(|c| if c.unit == UNIT_PICOWATT_HOURS { classify(&c.name) } else { None })
        .collect();
    let has_cores = |socket: i16| {
        channels.iter().zip(&roles).any(|(c, r)| *r == Some(Role::Cores) && socket_of(&c.name) == socket)
    };
    channels
        .iter()
        .zip(&roles)
        .enumerate()
        .filter_map(|(i, (c, role))| {
            let domain = match (*role)? {
                Role::Cores => Domain::Cpu,
                Role::Package if !has_cores(socket_of(&c.name)) => Domain::Cpu,
                Role::Package => return None,
                Role::Graphics => Domain::Igpu,
                Role::Dram => Domain::Dram,
            };
            Some((i, domain))
        })
        .collect()
}

#[cfg(windows)]
pub use device::EmiSensor;

#[cfg(windows)]
mod device {
    use std::ffi::c_void;
    use std::ptr;

    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInterfaceDetailW,
    };
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::core::GUID;

    use super::*;
    use crate::common::counter::reset_delta;
    use crate::domain::{Channel, Quality, Reading};
    use crate::{EnergySensor, SensorError};

    /// `GUID_DEVICE_ENERGY_METER` {45BD8344-7ED6-49cf-A440-C276C933B053}
    const GUID_DEVICE_ENERGY_METER: GUID = GUID::from_u128(0x45bd8344_7ed6_49cf_a440_c276c933b053);

    struct Device {
        handle: HANDLE,
        /// Channels in the device's measurement buffer.
        count: usize,
    }

    impl Drop for Device {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.handle) };
        }
    }

    /// Why no EMI sensor could be opened.
    #[derive(Debug)]
    pub enum OpenError {
        NoDevices,
        AccessDenied,
        NoUsableChannels(Vec<String>),
    }

    pub struct EmiSensor {
        devices: Vec<Device>,
        /// (device, channel within the device) per reported channel.
        sources: Vec<(usize, usize)>,
        channels: Vec<Channel>,
        prev_pwh: Vec<f64>,
        max_seen_pwh: Vec<f64>,
    }

    // Device handles are only used from the sampler's thread.
    unsafe impl Send for EmiSensor {}

    fn ioctl_buf(handle: HANDLE, code: u32, out: &mut [u8]) -> Option<usize> {
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                code,
                ptr::null(),
                0,
                out.as_mut_ptr() as *mut c_void,
                out.len() as u32,
                &mut returned,
                ptr::null_mut(),
            )
        };
        (ok != 0).then_some(returned as usize)
    }

    /// Device paths of every present EMI device.
    fn device_paths() -> Vec<Vec<u16>> {
        let mut paths = Vec::new();
        unsafe {
            let set = SetupDiGetClassDevsW(
                &GUID_DEVICE_ENERGY_METER,
                ptr::null(),
                ptr::null_mut(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            );
            if set == -1 {
                return paths;
            }
            for index in 0.. {
                let mut data: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
                data.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
                if SetupDiEnumDeviceInterfaces(set, ptr::null(), &GUID_DEVICE_ENERGY_METER, index, &mut data) == 0 {
                    break;
                }
                let mut required = 0u32;
                SetupDiGetDeviceInterfaceDetailW(set, &data, ptr::null_mut(), 0, &mut required, ptr::null_mut());
                if required == 0 {
                    continue;
                }
                // u32 storage keeps the detail struct aligned.
                let mut storage = vec![0u32; (required as usize).div_ceil(4)];
                let detail = storage.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
                (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
                if SetupDiGetDeviceInterfaceDetailW(set, &data, detail, required, ptr::null_mut(), ptr::null_mut()) == 0 {
                    continue;
                }
                let start = ptr::addr_of!((*detail).DevicePath) as *const u16;
                let max = (required as usize - 4) / 2;
                let path: Vec<u16> = (0..max).map(|i| *start.add(i)).take_while(|&c| c != 0).collect();
                paths.push(path);
            }
            SetupDiDestroyDeviceInfoList(set);
        }
        paths
    }

    impl EmiSensor {
        pub fn open() -> Result<Self, OpenError> {
            let paths = device_paths();
            if paths.is_empty() {
                return Err(OpenError::NoDevices);
            }
            let mut devices = Vec::new();
            let mut sources = Vec::new();
            let mut channels = Vec::new();
            let mut unused = Vec::new();
            let mut denied = false;
            for mut path in paths {
                path.push(0);
                let handle = unsafe {
                    CreateFileW(
                        path.as_ptr(),
                        GENERIC_READ,
                        FILE_SHARE_READ | FILE_SHARE_WRITE,
                        ptr::null(),
                        OPEN_EXISTING,
                        0,
                        ptr::null_mut(),
                    )
                };
                if handle == INVALID_HANDLE_VALUE {
                    denied |= std::io::Error::last_os_error().kind() == std::io::ErrorKind::PermissionDenied;
                    continue;
                }
                let mut device = Device { handle, count: 0 };
                let mut version = [0u8; 2];
                let Some(_) = ioctl_buf(handle, IOCTL_EMI_GET_VERSION, &mut version) else { continue };
                // EMI_METADATA_SIZE; a zeroed 4-byte buffer reads it whether the
                // field is 16 or 32 bits wide.
                let mut size = [0u8; 4];
                let Some(_) = ioctl_buf(handle, IOCTL_EMI_GET_METADATA_SIZE, &mut size) else { continue };
                let mut meta = vec![0u8; (u32::from_le_bytes(size) as usize).clamp(80, 1 << 16)];
                let Some(len) = ioctl_buf(handle, IOCTL_EMI_GET_METADATA, &mut meta) else { continue };
                let parsed = match u16::from_le_bytes(version) {
                    1 => parse_metadata_v1(&meta[..len]),
                    _ => parse_metadata_v2(&meta[..len]),
                };
                let Some(meta_channels) = parsed else { continue };
                let selected = select(&meta_channels);
                let index = devices.len();
                for (i, c) in meta_channels.iter().enumerate() {
                    match selected.iter().find(|(s, _)| *s == i) {
                        Some((_, domain)) => {
                            sources.push((index, i));
                            channels.push(Channel {
                                domain: domain.clone(),
                                socket: socket_of(&c.name),
                                name: c.name.clone(),
                                quality: Quality::Measured,
                            });
                        }
                        None => unused.push(c.name.clone()),
                    }
                }
                device.count = meta_channels.len();
                devices.push(device);
            }
            if channels.is_empty() {
                return Err(if denied {
                    OpenError::AccessDenied
                } else if unused.is_empty() {
                    OpenError::NoDevices
                } else {
                    OpenError::NoUsableChannels(unused)
                });
            }
            let mut sensor = EmiSensor {
                devices,
                sources,
                channels,
                prev_pwh: Vec::new(),
                max_seen_pwh: Vec::new(),
            };
            let first = sensor.measure().map_err(|_| OpenError::NoDevices)?;
            sensor.max_seen_pwh = first.clone();
            sensor.prev_pwh = first;
            Ok(sensor)
        }

        /// Cumulative pWh per reported channel.
        fn measure(&self) -> Result<Vec<f64>, SensorError> {
            let mut per_device = Vec::with_capacity(self.devices.len());
            for d in &self.devices {
                let mut buf = vec![0u8; d.count * 16];
                let len = ioctl_buf(d.handle, IOCTL_EMI_GET_MEASUREMENT, &mut buf)
                    .ok_or_else(|| SensorError::new("emi", std::io::Error::last_os_error().to_string()))?;
                per_device.push(
                    parse_measurements(&buf[..len], d.count)
                        .ok_or_else(|| SensorError::new("emi", "short measurement buffer"))?,
                );
            }
            Ok(self.sources.iter().map(|&(d, c)| per_device[d][c] as f64).collect())
        }
    }

    impl EnergySensor for EmiSensor {
        fn name(&self) -> &'static str {
            "emi"
        }

        fn channels(&self) -> &[Channel] {
            &self.channels
        }

        fn read(&mut self, _tree: &[u32]) -> Result<Vec<Reading>, SensorError> {
            let curr = self.measure()?;
            let readings = curr
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    let joules = pwh_to_joules(reset_delta(self.prev_pwh[i], c, self.max_seen_pwh[i]));
                    self.max_seen_pwh[i] = self.max_seen_pwh[i].max(c);
                    Reading::energy(joules)
                })
                .collect();
            self.prev_pwh = curr;
            Ok(readings)
        }
    }
}

#[cfg(windows)]
pub use device::OpenError;

#[cfg(test)]
mod tests {
    use super::*;

    fn wstr(s: &str) -> Vec<u8> {
        s.encode_utf16().chain([0]).flat_map(|u| u.to_le_bytes()).collect()
    }

    /// Builds an EMI_METADATA_V2 buffer.
    fn v2(channels: &[(&str, u32)]) -> Vec<u8> {
        let mut buf = vec![0u8; 64]; // OEM + model
        buf.extend(3u16.to_le_bytes()); // revision
        buf.extend((channels.len() as u16).to_le_bytes());
        for (name, unit) in channels {
            let w = wstr(name);
            buf.extend(unit.to_le_bytes());
            buf.extend((w.len() as u16).to_le_bytes());
            buf.extend(w);
        }
        buf
    }

    #[test]
    fn ioctl_codes_match_emi_h() {
        assert_eq!(IOCTL_EMI_GET_VERSION, 0x0022_4000);
        assert_eq!(IOCTL_EMI_GET_METADATA_SIZE, 0x0022_4004);
        assert_eq!(IOCTL_EMI_GET_METADATA, 0x0022_4008);
        assert_eq!(IOCTL_EMI_GET_MEASUREMENT, 0x0022_400C);
    }

    #[test]
    fn parses_v2_variable_size_channels() {
        let buf = v2(&[("RAPL_Package0_PKG", 0), ("RAPL_Package0_PP0", 0), ("RAPL_Package0_DRAM", 0)]);
        let ch = parse_metadata_v2(&buf).unwrap();
        let names: Vec<&str> = ch.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["RAPL_Package0_PKG", "RAPL_Package0_PP0", "RAPL_Package0_DRAM"]);
        assert!(parse_metadata_v2(&buf[..buf.len() - 3]).is_none(), "truncated buffer");
    }

    #[test]
    fn parses_v1_single_channel() {
        let mut buf = vec![0u8; 72];
        let name = wstr("CPU Cores");
        buf[70..72].copy_from_slice(&(name.len() as u16).to_le_bytes());
        buf.extend(name);
        assert_eq!(parse_metadata_v1(&buf).unwrap(), [EmiChannel { name: "CPU Cores".into(), unit: 0 }]);
    }

    #[test]
    fn parses_measurements() {
        let mut buf = Vec::new();
        for (e, t) in [(1000u64, 5u64), (2000, 6)] {
            buf.extend(e.to_le_bytes());
            buf.extend(t.to_le_bytes());
        }
        assert_eq!(parse_measurements(&buf, 2), Some(vec![1000, 2000]));
        assert_eq!(parse_measurements(&buf, 3), None);
    }

    #[test]
    fn picowatt_hours_to_joules() {
        assert_eq!(pwh_to_joules(1e12 / 3600.0), 1.0);
        assert_eq!(pwh_to_joules(2_777_777.0), 2_777_777.0 * 3600.0 / 1e12);
    }

    #[test]
    fn selects_cores_over_package_and_skips_psys() {
        let ch = parse_metadata_v2(&v2(&[
            ("RAPL_Package0_PKG", 0),
            ("RAPL_Package0_PP0", 0),
            ("RAPL_Package0_PP1", 0),
            ("RAPL_Package0_DRAM", 0),
            ("RAPL_PSYS", 0),
            ("RAPL_Package1_PKG", 0), // socket without a core channel → package
            ("Weird", 0),
            ("RAPL_Package2_PP0", 7), // unknown unit
        ]))
        .unwrap();
        assert_eq!(
            select(&ch),
            [(1, Domain::Cpu), (2, Domain::Igpu), (3, Domain::Dram), (5, Domain::Cpu)]
        );
        assert_eq!(socket_of("RAPL_Package1_PKG"), 1);
    }
}
