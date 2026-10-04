//! Per-GPU energy counter.

use nvml_wrapper::Device;

/// Returns the total energy consumption counter for a single GPU device in millijoules (mJ).
/// The counter is monotonically increasing and resets only on driver reload or GPU reset.
pub fn get_gpu_energy(device: &Device) -> Result<f64, Box<dyn std::error::Error>> {
    let energy_milij = device.total_energy_consumption()?; // in milliJ
    Ok(energy_milij as f64)
}
