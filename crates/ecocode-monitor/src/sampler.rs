//! The single measurement loop, used by every `ecocode` command.
//!
//! On every tick the [`Sampler`] reads all energy sensors, measures the process
//! tree's CPU and memory share, attributes energy to it, attaches the latest
//! markers and produces one [`Record`]. [`run_loop`] drives it at a fixed
//! interval until the caller says stop.

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::Utc;
use ecocode_sensors::common::process_tree::get_process_tree_pids_from_sys;
use ecocode_sensors::{Domain, EnergySensor, ProcessInspector, Reading};
use sysinfo::{Pid, ProcessesToUpdate, RefreshKind, System};

use crate::attribution::{Usage, gpu_process_share, share_energy};
use crate::markers::MarkerState;
use crate::record::{GpuMetric, Record, SocketMetric};
use crate::sink::Sink;

/// Wall-clock time and interval length of one tick, captured before any
/// sensor work so consecutive timestamps are evenly spaced.
#[derive(Clone, Debug)]
pub struct Tick {
    pub wall: String,
    pub elapsed_secs: f64,
}

pub struct Sampler {
    sensors: Vec<Box<dyn EnergySensor>>,
    inspector: Box<dyn ProcessInspector>,
    sys: System,
    root: u32,
    run_id: i64,
    num_cores: usize,
    total_memory: u64,
    prev_cpu: HashMap<u32, u64>,
    prev_cpu_at: Instant,
    /// Per-process CPU energy (nJ) at the previous tick, where the OS measures it.
    prev_energy: Option<HashMap<u32, u64>>,
    last_tick: Instant,
    iteration: u32,
    /// Sensors whose read failed, so each failure is reported once.
    failed: BTreeMap<&'static str, String>,
}

impl Sampler {
    /// Creates the sampler and takes the warm-up readings.
    ///
    /// The warm-up fixes three first-sample artefacts:
    ///   1. NVML would return ALL historical utilisation data on the first query,
    ///      not just the last interval → inflated process share on sample 1.
    ///   2. Energy counters include the process startup burst (CUDA context
    ///      init, library loading) which would corrupt the first energy delta.
    ///   3. The process table needs a refresh before the process tree
    ///      (children/threads) can be discovered for the CPU-time baseline.
    pub fn new(
        sensors: Vec<Box<dyn EnergySensor>>,
        inspector: Box<dyn ProcessInspector>,
        root_pid: u32,
        run_id: i64,
    ) -> Self {
        let sys = System::new_with_specifics(RefreshKind::everything());
        let num_cores = sys.cpus().len();
        let total_memory = sys.total_memory();
        let mut sampler = Sampler {
            sensors,
            inspector,
            sys,
            root: root_pid,
            run_id,
            num_cores,
            total_memory,
            prev_cpu: HashMap::new(),
            prev_cpu_at: Instant::now(),
            prev_energy: None,
            last_tick: Instant::now(),
            iteration: 0,
            failed: BTreeMap::new(),
        };
        sampler
            .sys
            .refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(root_pid)]), true);
        let tree = get_process_tree_pids_from_sys(&sampler.sys, root_pid);
        for sensor in &mut sampler.sensors {
            let _ = sensor.read(&tree);
        }
        sampler.prev_cpu = sampler.inspector.cpu_times(&sampler.sys, &tree);
        sampler.prev_energy = sampler.inspector.cpu_energy_nj(&tree);
        sampler.prev_cpu_at = Instant::now();
        sampler.last_tick = Instant::now();
        sampler
    }

    /// The process table, refreshed every tick (also holds CPU and memory info).
    pub fn system(&self) -> &System {
        &self.sys
    }

    pub fn sensors(&self) -> &[Box<dyn EnergySensor>] {
        &self.sensors
    }

    pub fn set_run_id(&mut self, run_id: i64) {
        self.run_id = run_id;
    }

    /// Starts a tick: captures the timestamp and the time since the previous tick.
    pub fn begin_tick(&mut self) -> Tick {
        let wall = Utc::now().to_rfc3339();
        let now = Instant::now();
        let elapsed_secs = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;
        Tick { wall, elapsed_secs }
    }

    /// Refreshes the process table. Call once per tick, before [`root_alive`](Self::root_alive).
    pub fn refresh(&mut self) {
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
    }

    /// The root process and all its descendants, as of the last refresh.
    pub fn tree(&self) -> Vec<u32> {
        get_process_tree_pids_from_sys(&self.sys, self.root)
    }

    /// Whether the monitored root process still exists (as of the last refresh).
    pub fn root_alive(&self) -> bool {
        self.sys.process(Pid::from_u32(self.root)).is_some()
    }

    /// Sensor read failures so far, one message per sensor.
    pub fn sensor_failures(&self) -> &BTreeMap<&'static str, String> {
        &self.failed
    }

    /// Measures the interval that ended at `tick` and builds its record.
    pub fn sample(&mut self, tick: Tick, markers: &Mutex<MarkerState>) -> Record {
        self.iteration += 1;

        // Build the full process tree (root + all descendants) so that child
        // processes (data-loader workers, torch.distributed ranks, etc.) are
        // included in CPU, memory, and GPU attribution.
        let tree = get_process_tree_pids_from_sys(&self.sys, self.root);

        // CPU: interval average from cumulative CPU-time deltas.
        let now_cpu = Instant::now();
        let elapsed_units =
            now_cpu.duration_since(self.prev_cpu_at).as_secs_f64() * self.inspector.cpu_time_units_per_sec();
        let curr_cpu = self.inspector.cpu_times(&self.sys, &tree);
        let mut total_delta = 0_u64;
        for (p, &curr) in &curr_cpu {
            if let Some(&prev) = self.prev_cpu.get(p) {
                total_delta += curr.saturating_sub(prev);
            }
        }
        let cpu_usage = if elapsed_units > 0.0 && self.num_cores > 0 {
            ((total_delta as f64 / elapsed_units) * 100.0 / self.num_cores as f64).min(100.0) as f32
        } else {
            0.0
        };
        self.prev_cpu = curr_cpu;
        self.prev_cpu_at = now_cpu;

        // Memory share of total physical memory.
        let mem_bytes = self.inspector.memory_bytes(&self.sys, &tree);
        let mem_usage = if self.total_memory > 0 {
            ((mem_bytes as f64 / self.total_memory as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        // Per-process CPU energy, where the OS measures it (Apple Silicon).
        let direct_cpu_j = self.inspector.cpu_energy_nj(&tree).map(|curr| {
            let prev = self.prev_energy.take().unwrap_or_default();
            // A process missing from the previous tick started since, so all
            // of its energy belongs to this interval.
            let nj: u64 = curr
                .iter()
                .map(|(pid, &e)| e.saturating_sub(prev.get(pid).copied().unwrap_or(0)))
                .sum();
            self.prev_energy = Some(curr);
            nj as f64 / 1e9
        });

        let usage = Usage {
            cpu_pct: cpu_usage as f64,
            mem_pct: mem_usage,
            igpu_pct: cpu_usage as f64, // igpu proxy: no per-process iGPU sensor available
        };

        let mut cpu_j = 0.0_f64;
        let mut mem_j: Option<f64> = None;
        let mut igpu_j: Option<f64> = None;
        let mut socket_metrics = Vec::new();
        let mut gpu_metrics = Vec::new();
        let mut total_gpu_energy_j = 0.0_f64;
        let mut total_gpu_util = 0.0_f64;
        let mut direct_cpu_used = false;

        for sensor in &mut self.sensors {
            let result = sensor.read(&tree);
            let channels = sensor.channels();
            let readings = match result {
                Ok(r) if r.len() == channels.len() => r,
                Ok(r) => {
                    let msg = format!("returned {} readings for {} channels", r.len(), channels.len());
                    report_failure(&mut self.failed, sensor.name(), msg);
                    vec![Reading::default(); channels.len()]
                }
                Err(e) => {
                    report_failure(&mut self.failed, sensor.name(), e.message);
                    vec![Reading::default(); channels.len()]
                }
            };
            for (channel, reading) in channels.iter().zip(readings) {
                match &channel.domain {
                    Domain::Gpu(index) => {
                        let (share, util) = match (&reading.tree_share, reading.util_samples.as_deref()) {
                            (Some(ts), _) => (ts.share, ts.util_pct),
                            (None, Some(samples)) => gpu_process_share(samples, &tree),
                            (None, None) => (0.0, 0.0),
                        };
                        let energy = reading.joules * share;
                        total_gpu_energy_j += energy;
                        total_gpu_util += util;
                        gpu_metrics.push(GpuMetric {
                            device_index: *index,
                            gpu_usage: util,
                            gpu_energy: energy,
                        });
                    }
                    domain => {
                        let energy = match (domain, direct_cpu_j, &reading.tree_share) {
                            (_, _, Some(ts)) => (reading.joules * ts.share).max(0.0),
                            // The OS-measured energy covers the whole process tree, so it
                            // goes to the first CPU channel only, capped at what the
                            // package measured over the same interval.
                            (Domain::Cpu, Some(direct), None) => {
                                let first = !std::mem::replace(&mut direct_cpu_used, true);
                                match (first, reading.joules > 0.0) {
                                    (false, _) => 0.0,
                                    (true, true) => direct.min(reading.joules),
                                    (true, false) => direct,
                                }
                            }
                            _ => share_energy(domain, reading.joules, &usage),
                        };
                        match domain {
                            Domain::Cpu => cpu_j += energy,
                            Domain::Dram => mem_j = Some(mem_j.unwrap_or(0.0) + energy),
                            Domain::Igpu => igpu_j = Some(igpu_j.unwrap_or(0.0) + energy),
                            _ => {}
                        }
                        socket_metrics.push(SocketMetric {
                            socket: channel.socket,
                            domain: domain.label(),
                            energy,
                        });
                    }
                }
            }
        }

        // OS-measured process energy with no CPU energy sensor to put it on.
        if let (Some(direct), false) = (direct_cpu_j, direct_cpu_used)
            && direct > 0.0
        {
            cpu_j += direct;
            socket_metrics.push(SocketMetric { socket: 0, domain: Domain::Cpu.label(), energy: direct });
        }

        let avg_gpu_util = if gpu_metrics.is_empty() {
            0.0
        } else {
            total_gpu_util / gpu_metrics.len() as f64
        };

        let state = markers.lock().unwrap_or_else(|e| e.into_inner()).clone();

        Record {
            id: self.iteration,
            run_id: self.run_id,
            pid: self.root,
            timestamp: tick.wall,
            elapsed_secs: tick.elapsed_secs,
            cpu_usage: cpu_usage as f64,
            cpu_energy: cpu_j,
            gpu_usage: avg_gpu_util,
            gpu_energy: total_gpu_energy_j,
            mem_usage: (mem_usage * 100.0).round() / 100.0,
            mem_energy: mem_j,
            igpu_usage: None,
            igpu_energy: igpu_j,
            gpu_metrics,
            socket_metrics,
            phase: state.current_phase.clone(),
            tokens_processed: state.tokens_processed,
            training_step: state.training_step,
            loss: state.loss,
            learning_rate: state.learning_rate,
            sequence_length: state.sequence_length,
            accuracy: state.accuracy,
            custom_metrics: state.custom_metrics_opt(),
        }
    }
}

fn report_failure(failed: &mut BTreeMap<&'static str, String>, sensor: &'static str, message: String) {
    if !failed.contains_key(sensor) {
        eprintln!("Warning: could not read {sensor} sensor ({message}); reporting 0 J for it.");
        failed.insert(sensor, message);
    }
}

/// What the loop should do on this tick, decided after the process table refresh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// Sample and keep going.
    Continue,
    /// Take one final sample, then stop.
    LastSample,
    /// Stop without sampling (e.g. the monitored process exited).
    Stop,
}

/// Why [`run_loop`] returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopEnd {
    /// `check` asked to stop.
    Finished,
    /// The stop flag was set (Ctrl+C).
    Interrupted,
}

/// Samples every `interval` until `check` returns [`Control::Stop`] or
/// [`Control::LastSample`], or `stop` is set. Sink errors end the loop.
///
/// A late tick (more than 5 ms past its deadline) schedules the next one a
/// full interval after it fired, rather than bursting to catch up.
pub fn run_loop(
    sampler: &mut Sampler,
    interval: Duration,
    stop: &AtomicBool,
    markers: &Mutex<MarkerState>,
    sink: &mut dyn Sink,
    mut check: impl FnMut(&Sampler) -> Control,
) -> Result<LoopEnd, Box<dyn Error>> {
    let mut deadline = Instant::now() + interval;
    loop {
        if wait_until(deadline, stop) {
            return Ok(LoopEnd::Interrupted);
        }
        let fired = Instant::now();
        deadline = if fired > deadline + Duration::from_millis(5) {
            fired + interval
        } else {
            deadline + interval
        };

        let tick = sampler.begin_tick();
        sampler.refresh();
        let control = check(sampler);
        if control == Control::Stop {
            return Ok(LoopEnd::Finished);
        }
        let record = sampler.sample(tick, markers);
        sink.write(&record)?;
        if control == Control::LastSample {
            return Ok(LoopEnd::Finished);
        }
    }
}

/// Sleeps until `deadline`; returns true early if `stop` is set.
fn wait_until(deadline: Instant, stop: &AtomicBool) -> bool {
    const SLICE: Duration = Duration::from_millis(20);
    loop {
        if stop.load(Ordering::SeqCst) {
            return true;
        }
        let now = Instant::now();
        if now >= deadline {
            return false;
        }
        std::thread::sleep((deadline - now).min(SLICE));
    }
}
