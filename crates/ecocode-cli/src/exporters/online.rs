//! Online exporter — sends measurement records to the EcoCode web platform via REST API.
//! Requires a workspace-scoped API key (`ecocode_...`) for authentication.
//!
//! The sampling loop never waits on the network: records go into a bounded
//! [`UploadQueue`] and a background thread posts them. If the backend is slow
//! or down, the oldest queued records are dropped (and counted) rather than
//! delaying measurements.

use std::collections::VecDeque;
use std::env;
use std::error::Error;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ecocode_api::EcoCodeHttpClient;
use ecocode_api::types::{GpuMetric as WireGpuMetric, IngestRecord, SocketMetric as WireSocketMetric};

use crate::config;
use crate::exporters::{CarbonConfig, Exporter, ExporterType, ProjectInfo, Record, SystemInfo};

/// Records waiting to be uploaded; beyond this the oldest are dropped.
pub const QUEUE_CAPACITY: usize = 300;
/// How long the end of a run waits for queued records to be uploaded.
pub const FLUSH_TIMEOUT: Duration = Duration::from_secs(10);

pub struct OnlineExporter {
    client: Arc<EcoCodeHttpClient>,
    queue: Option<UploadQueue<(i64, IngestRecord)>>,
    pub exporter_type_label: String,
}

impl OnlineExporter {
    /// Resolves the API key in priority order:
    /// 1. Command-line argument (--api-key)
    /// 2. Environment variable (ECOCODE_API_KEY)
    /// 3. Stored config file (~/.ecocode/config.json)
    pub fn new(api_key_arg: Option<String>) -> Result<Self, Box<dyn Error>> {
        let config = config::load_config().ok();

        let api_key = api_key_arg
            .clone()
            .or_else(|| env::var("ECOCODE_API_KEY").ok())
            .or_else(|| config.as_ref().and_then(|c| c.api_key.clone()))
            .ok_or(
                "Error: No API key found.\n\
                 EcoCode requires an account to record runs.\n\
                 \n\
                 Sign up on the EcoCode web platform and get your API key under Settings > Developer.\n\
                 \n\
                 Then provide it via:\n\
                 1. Command line:        --api-key <KEY>\n\
                 2. Environment variable: ECOCODE_API_KEY=<KEY>\n\
                 3. Save it once:        ecocode config set-api-key <KEY>",
            )?;

        // If API key was provided via command line, offer to save it
        if api_key_arg.is_some() && config.as_ref().and_then(|c| c.api_key.as_ref()) != Some(&api_key) {
            let masked = if api_key.len() > 12 {
                format!("{}...{}", &api_key[..8], &api_key[api_key.len() - 4..])
            } else {
                "****".to_string()
            };
            println!("💡 Tip: Save your API key to avoid entering it each time:");
            println!("   ecocode config set-api-key <YOUR_KEY>");
            println!("   (detected key: {})", masked);
        }

        Self::with_key(&api_key)
    }

    /// Exporter for an explicit API key, without the save-key tip.
    pub fn with_key(api_key: &str) -> Result<Self, Box<dyn Error>> {
        let client = EcoCodeHttpClient::new(api_key).map_err(|e| e.to_string())?;
        Ok(Self::with_client(client))
    }

    pub fn with_client(client: EcoCodeHttpClient) -> Self {
        OnlineExporter {
            client: Arc::new(client),
            queue: None,
            exporter_type_label: "online".to_string(),
        }
    }

    fn queue(&mut self) -> &mut UploadQueue<(i64, IngestRecord)> {
        let client = Arc::clone(&self.client);
        self.queue.get_or_insert_with(|| {
            UploadQueue::start(QUEUE_CAPACITY, move |(run_id, record): &(i64, IngestRecord)| {
                client.add_record(*run_id, record).map_err(|e| e.to_string())
            })
        })
    }
}

/// The backend wire format for one record.
pub fn to_ingest(r: &Record) -> IngestRecord {
    IngestRecord {
        pid: r.pid,
        timestamp: r.timestamp.clone(),
        elapsed_secs: Some(r.elapsed_secs),
        cpu_usage: r.cpu_usage,
        cpu_energy: r.cpu_energy,
        gpu_usage: r.gpu_usage,
        gpu_energy: r.gpu_energy,
        mem_usage: r.mem_usage,
        mem_energy: r.mem_energy,
        igpu_usage: r.igpu_usage,
        igpu_energy: r.igpu_energy,
        gpu_metrics: r
            .gpu_metrics
            .iter()
            .map(|g| WireGpuMetric { device_index: g.device_index, gpu_usage: g.gpu_usage, gpu_energy: g.gpu_energy })
            .collect(),
        socket_metrics: r
            .socket_metrics
            .iter()
            .map(|s| WireSocketMetric { socket: s.socket, domain: s.domain.clone(), energy: s.energy })
            .collect(),
        phase: r.phase.clone(),
        epoch_num: None,
        tokens_processed: r.tokens_processed,
        training_step: r.training_step,
        loss: r.loss,
        learning_rate: r.learning_rate,
        sequence_length: r.sequence_length,
        accuracy: r.accuracy,
        custom_metrics: r.custom_metrics.clone(),
    }
}

impl Exporter for OnlineExporter {
    fn exporter_type(&self) -> ExporterType {
        ExporterType::Online
    }

    fn add_record(&mut self, record: Record) -> Result<(), Box<dyn Error>> {
        let item = (record.run_id, to_ingest(&record));
        self.queue().push(item);
        Ok(())
    }

    /// Waits (up to [`FLUSH_TIMEOUT`]) for queued records to be uploaded.
    fn export(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(queue) = self.queue.take() {
            let stats = queue.finish(FLUSH_TIMEOUT);
            if stats.lost() > 0 {
                eprintln!(
                    "Warning: {} of {} records were not uploaded ({} dropped while the backend was slow, \
                     {} failed, {} still queued at exit).{}",
                    stats.lost(),
                    stats.total(),
                    stats.dropped,
                    stats.failed,
                    stats.unsent,
                    stats.last_error.map(|e| format!(" Last error: {e}")).unwrap_or_default()
                );
            }
        }
        Ok(())
    }

    fn export_line(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn get_or_create_project(
        &mut self,
        project_name: &str,
        collaborator_name: Option<&str>,
    ) -> Result<ProjectInfo, Box<dyn Error>> {
        // Fetch all projects and find the one matching by name.
        let projects = self.client.list_projects().map_err(|e| e.to_string())?;
        let project = projects
            .into_iter()
            .find(|p| p.name.as_deref() == Some(project_name))
            .ok_or_else(|| {
                format!(
                    "Project '{}' not found in your workspaces. Please create it first in the web app.",
                    project_name
                )
            })?;

        Ok(ProjectInfo {
            id: project.id,
            selected_collaborator_name: collaborator_name.map(|s| s.to_string()),
            collaborators: vec![],
            is_new: false,
        })
    }

    fn create_run(
        &mut self,
        run_name: &str,
        project_id: i64,
        collaborator_name: Option<&str>,
        started_at: Option<&str>,
        command: Option<&str>,
    ) -> Result<i64, Box<dyn Error>> {
        Ok(self
            .client
            .create_run(project_id, run_name, collaborator_name, started_at, command, &self.exporter_type_label)
            .map_err(|e| e.to_string())?)
    }

    fn save_system_info(&mut self, run_id: i64, info: SystemInfo) -> Result<(), Box<dyn Error>> {
        Ok(self.client.save_system_info(run_id, &info).map_err(|e| e.to_string())?)
    }

    fn complete_run(&mut self, run_id: i64, status: &str, ended_at: Option<&str>) -> Result<(), Box<dyn Error>> {
        Ok(self.client.complete_run(run_id, status, ended_at).map_err(|e| e.to_string())?)
    }

    fn run_summary(&mut self, _run_id: i64) -> Result<(), Box<dyn Error>> {
        // Summary is calculated server-side in this exporter
        Ok(())
    }

    fn get_carbon_config(&mut self, project_id: i64) -> Result<CarbonConfig, Box<dyn Error>> {
        Ok(self
            .client
            .carbon_config(project_id)
            .map_err(|e| e.to_string())?
            .unwrap_or_else(CarbonConfig::global_fallback))
    }
}

/// Upload outcome counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UploadStats {
    pub sent: u64,
    pub failed: u64,
    /// Dropped because the queue was full.
    pub dropped: u64,
    /// Still queued when the flush timed out.
    pub unsent: u64,
    pub last_error: Option<String>,
}

impl UploadStats {
    pub fn total(&self) -> u64 {
        self.sent + self.lost()
    }
    pub fn lost(&self) -> u64 {
        self.failed + self.dropped + self.unsent
    }
}

struct QueueState<T> {
    items: VecDeque<T>,
    in_flight: bool,
    closed: bool,
    stats: UploadStats,
}

/// Bounded FIFO drained by one background thread. `push` never blocks.
pub struct UploadQueue<T> {
    shared: Arc<(Mutex<QueueState<T>>, Condvar)>,
    capacity: usize,
}

impl<T: Send + 'static> UploadQueue<T> {
    pub fn start(capacity: usize, mut send: impl FnMut(&T) -> Result<(), String> + Send + 'static) -> Self {
        let shared = Arc::new((
            Mutex::new(QueueState {
                items: VecDeque::new(),
                in_flight: false,
                closed: false,
                stats: UploadStats::default(),
            }),
            Condvar::new(),
        ));
        let worker = Arc::clone(&shared);
        thread::spawn(move || {
            let (lock, cvar) = &*worker;
            loop {
                let item = {
                    let mut st = lock.lock().unwrap();
                    while st.items.is_empty() && !st.closed {
                        st = cvar.wait(st).unwrap();
                    }
                    match st.items.pop_front() {
                        Some(item) => {
                            st.in_flight = true;
                            item
                        }
                        None => return, // closed and drained
                    }
                };
                let result = send(&item);
                let mut st = lock.lock().unwrap();
                st.in_flight = false;
                match result {
                    Ok(()) => st.stats.sent += 1,
                    Err(e) => {
                        if st.stats.failed == 0 {
                            eprintln!("[ecocode] Failed to send record: {e}");
                        }
                        st.stats.failed += 1;
                        st.stats.last_error = Some(e);
                    }
                }
                cvar.notify_all();
            }
        });
        UploadQueue { shared, capacity }
    }

    /// Queues an item, dropping the oldest one if the queue is full.
    pub fn push(&self, item: T) {
        let (lock, cvar) = &*self.shared;
        let mut st = lock.lock().unwrap();
        if st.items.len() >= self.capacity {
            st.items.pop_front();
            st.stats.dropped += 1;
        }
        st.items.push_back(item);
        cvar.notify_all();
    }

    /// Stops accepting items and waits up to `timeout` for the queue to drain.
    pub fn finish(self, timeout: Duration) -> UploadStats {
        let (lock, cvar) = &*self.shared;
        let deadline = Instant::now() + timeout;
        let mut st = lock.lock().unwrap();
        st.closed = true;
        cvar.notify_all();
        while !st.items.is_empty() || st.in_flight {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            st = cvar.wait_timeout(st, deadline - now).unwrap().0;
        }
        let mut stats = st.stats.clone();
        stats.unsent = st.items.len() as u64 + st.in_flight as u64;
        // Anything left is abandoned; the worker exits once it drains or the process ends.
        st.items.clear();
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn push_never_waits_for_a_slow_backend() {
        let q = UploadQueue::start(5, |_: &u32| {
            thread::sleep(Duration::from_millis(200));
            Ok(())
        });
        let started = Instant::now();
        for i in 0..20 {
            q.push(i);
        }
        assert!(started.elapsed() < Duration::from_millis(50), "push blocked for {:?}", started.elapsed());
        let stats = q.finish(Duration::from_secs(5));
        // Five fit in the queue (plus at most one already in flight); the rest were dropped.
        assert_eq!(stats.sent + stats.dropped, 20, "{stats:?}");
        assert!((5..=6).contains(&stats.sent), "{stats:?}");
        assert_eq!(stats.lost(), stats.dropped);
    }

    #[test]
    fn finish_drains_everything_when_fast() {
        let sent = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&sent);
        let q = UploadQueue::start(100, move |_: &u32| {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        for i in 0..50 {
            q.push(i);
        }
        let stats = q.finish(Duration::from_secs(5));
        assert_eq!(stats, UploadStats { sent: 50, ..Default::default() });
        assert_eq!(sent.load(Ordering::SeqCst), 50);
    }

    #[test]
    fn finish_times_out_and_counts_unsent() {
        let q = UploadQueue::start(100, |_: &u32| {
            thread::sleep(Duration::from_millis(300));
            Ok(())
        });
        for i in 0..10 {
            q.push(i);
        }
        let started = Instant::now();
        let stats = q.finish(Duration::from_millis(400));
        assert!(started.elapsed() < Duration::from_millis(900));
        assert_eq!(stats.sent + stats.unsent, 10);
        assert!(stats.unsent >= 8, "{stats:?}");
    }

    #[test]
    fn failures_are_counted_with_the_last_error() {
        let q = UploadQueue::start(10, |i: &u32| if i.is_multiple_of(2) { Err(format!("bad {i}")) } else { Ok(()) });
        for i in 0..4 {
            q.push(i);
        }
        let stats = q.finish(Duration::from_secs(5));
        assert_eq!((stats.sent, stats.failed), (2, 2));
        assert_eq!(stats.last_error.as_deref(), Some("bad 2"));
    }

    /// A slow mock backend: records keep flowing to the exporter at full speed
    /// and are all delivered once the backend catches up.
    #[test]
    fn exporter_uploads_to_a_slow_backend_in_the_background() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let received = Arc::new(Mutex::new(Vec::<String>::new()));
        let log = Arc::clone(&received);
        thread::spawn(move || {
            for mut req in server.incoming_requests() {
                thread::sleep(Duration::from_millis(100));
                let mut body = String::new();
                req.as_reader().read_to_string(&mut body).unwrap();
                log.lock().unwrap().push(format!("{} {}", req.url(), body));
                req.respond(tiny_http::Response::empty(200)).unwrap();
            }
        });

        let client = EcoCodeHttpClient::with_base_url(&url, "ecocode_test").unwrap();
        let mut exporter = OnlineExporter::with_client(client);
        let mut record: Record = serde_json::from_str(
            r#"{"id":1,"run_id":9,"pid":5,"timestamp":"t","elapsed_secs":1.0,"cpu_usage":1.0,"cpu_energy":2.0,
                "gpu_usage":0.0,"gpu_energy":0.0,"mem_usage":3.0,"mem_energy":null,"igpu_usage":null,
                "igpu_energy":null,"gpu_metrics":[],"socket_metrics":[],"phase":"eval","tokens_processed":null,
                "training_step":null,"loss":null,"learning_rate":null,"sequence_length":null,"accuracy":0.5}"#,
        )
        .unwrap();
        let started = Instant::now();
        for i in 0..5 {
            record.id = i;
            exporter.add_record(record.clone()).unwrap();
        }
        assert!(started.elapsed() < Duration::from_millis(50));
        exporter.export().unwrap();

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 5);
        assert!(received[0].starts_with("/api/ingest/run/9/record "));
        assert!(received[0].contains(r#""phase":"eval""#) && received[0].contains(r#""accuracy":0.5"#));
    }
}
