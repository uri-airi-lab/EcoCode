//! The sampling loop driven by fake sensors, monitoring this test process.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ecocode_monitor::{Control, LoopEnd, MarkerState, Record, Sampler, run_loop};
use ecocode_sensors::fake::{FAKE_GPU_UTIL, FAKE_WATTS};
use ecocode_sensors::{SensorMode, detect};

fn fake_sampler() -> Sampler {
    let d = detect(SensorMode::Fake);
    Sampler::new(d.sensors, d.inspector, std::process::id(), 7)
}

fn burn(ms: u64) {
    let start = Instant::now();
    let mut x = 0u64;
    while start.elapsed() < Duration::from_millis(ms) {
        x = x.wrapping_add(std::hint::black_box(1));
    }
    std::hint::black_box(x);
}

#[test]
fn record_attributes_fake_energy_to_this_process() {
    let mut sampler = fake_sampler();
    burn(200);
    let markers = Mutex::new(MarkerState::default());
    markers.lock().unwrap().apply(&serde_json::json!({"event": "phase", "value": "forward"}));
    let tick = sampler.begin_tick();
    sampler.refresh();
    assert!(sampler.root_alive());
    let r = sampler.sample(tick, &markers);

    assert_eq!((r.id, r.run_id, r.pid), (1, 7, std::process::id()));
    assert_eq!(r.phase.as_deref(), Some("forward"));
    assert!(r.cpu_usage > 0.0, "busy loop should show CPU usage");
    assert!(r.mem_usage > 0.0);

    // CPU energy is the fake cpu power times this process's CPU share.
    let cpu_wall = r.socket_metrics.iter().find(|s| s.domain == "cpu").unwrap().energy;
    assert_eq!(cpu_wall, r.cpu_energy);
    assert!(r.cpu_energy > 0.0 && r.cpu_energy <= FAKE_WATTS[0] * 1.0);
    assert!(r.mem_energy.unwrap() > 0.0);
    assert_eq!(r.igpu_energy, None);

    // The fake GPU splits activity evenly with one other process.
    assert_eq!(r.gpu_metrics.len(), 1);
    assert_eq!(r.gpu_usage, FAKE_GPU_UTIL as f64);
    let gpu_secs = r.gpu_energy / (FAKE_WATTS[2] * 0.5);
    assert!((0.15..1.0).contains(&gpu_secs), "gpu energy covers {gpu_secs}s");
}

#[test]
fn loop_samples_at_the_interval_until_told_to_stop() {
    let mut sampler = fake_sampler();
    let stop = AtomicBool::new(false);
    let markers = Mutex::new(MarkerState::default());
    let mut records: Vec<Record> = Vec::new();
    let mut ticks = 0;
    let end = run_loop(&mut sampler, Duration::from_millis(100), &stop, &markers, &mut records, |_| {
        ticks += 1;
        if ticks == 5 { Control::LastSample } else { Control::Continue }
    })
    .unwrap();

    assert_eq!(end, LoopEnd::Finished);
    assert_eq!(records.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    // Never early. A single tick may land late on a busy machine (the loop then
    // reschedules from the late tick), so bound the total rather than each one.
    let elapsed: Vec<f64> = records.iter().map(|r| r.elapsed_secs).collect();
    assert!(elapsed.iter().all(|&e| e >= 0.08), "elapsed {elapsed:?}");
    let total: f64 = elapsed.iter().sum();
    assert!((0.45..1.0).contains(&total), "5 ticks of 100 ms took {total}s: {elapsed:?}");
}

#[test]
fn stop_control_ends_without_a_sample() {
    let mut sampler = fake_sampler();
    let mut records: Vec<Record> = Vec::new();
    let end = run_loop(
        &mut sampler,
        Duration::from_millis(20),
        &AtomicBool::new(false),
        &Mutex::new(MarkerState::default()),
        &mut records,
        |_| Control::Stop,
    )
    .unwrap();
    assert_eq!(end, LoopEnd::Finished);
    assert!(records.is_empty());
}

#[test]
fn stop_flag_interrupts_a_long_wait() {
    let mut sampler = fake_sampler();
    let stop = AtomicBool::new(false);
    let mut records: Vec<Record> = Vec::new();
    let started = Instant::now();
    std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            stop.store(true, Ordering::SeqCst);
        });
        let end = run_loop(
            &mut sampler,
            Duration::from_secs(30),
            &stop,
            &Mutex::new(MarkerState::default()),
            &mut records,
            |_| Control::Continue,
        )
        .unwrap();
        assert_eq!(end, LoopEnd::Interrupted);
    });
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn no_sensors_still_produces_records() {
    let d = detect(SensorMode::None);
    let mut sampler = Sampler::new(d.sensors, d.inspector, std::process::id(), 0);
    let tick = sampler.begin_tick();
    sampler.refresh();
    let r = sampler.sample(tick, &Mutex::new(MarkerState::default()));
    assert_eq!((r.cpu_energy, r.gpu_energy, r.mem_energy), (0.0, 0.0, None));
    assert!(r.gpu_metrics.is_empty() && r.socket_metrics.is_empty());
}
