use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone)]
pub struct MemoryUsage {
    pub total: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(Debug, Clone)]
pub struct NetworkUsage {
    pub name: String,
    pub received: u64,
    pub transmitted: u64,
    /// Bytes per second; None until two valid counter samples are available.
    pub rates: Option<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub uptime: u64,
    pub logical_cpus: usize,
}

#[derive(Default)]
pub struct NetworkSampler {
    previous: BTreeMap<String, (u64, u64)>,
}

impl NetworkSampler {
    pub fn sample(
        &mut self,
        counters: BTreeMap<String, (u64, u64)>,
        elapsed: Duration,
    ) -> Vec<NetworkUsage> {
        let seconds = elapsed.as_secs_f64();
        let samples = counters
            .iter()
            .map(|(name, &(rx, tx))| {
                let rates = self.previous.get(name).and_then(|&(old_rx, old_tx)| {
                    if seconds == 0.0 {
                        return None;
                    }
                    Some((
                        rx.checked_sub(old_rx)? as f64 / seconds,
                        tx.checked_sub(old_tx)? as f64 / seconds,
                    ))
                });
                NetworkUsage {
                    name: name.clone(),
                    received: rx,
                    transmitted: tx,
                    rates,
                }
            })
            .collect();
        self.previous = counters;
        samples
    }
}

#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    pub ready: bool,
    pub cpu: Option<f32>,
    pub cores: Vec<f32>,
    pub disks: Vec<DiskUsage>,
    pub memory: Option<MemoryUsage>,
    pub networks: Vec<NetworkUsage>,
    pub system: Option<SystemInfo>,
    pub sampled_at: Instant,
    pub now: DateTime<Local>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            ready: false,
            cpu: None,
            cores: vec![],
            disks: vec![],
            memory: None,
            networks: vec![],
            system: None,
            sampled_at: Instant::now(),
            now: Local::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Cpu,
    Memory,
    Network,
    Storage,
    System,
}

pub type TileKey = (String, String, String);

#[derive(Debug, Clone)]
pub struct SampleRequest {
    pub key: TileKey,
    pub generation: u64,
    pub sources: &'static [Source],
}

pub struct SampleResult {
    pub request: SampleRequest,
    pub metrics: Metrics,
}

/// The UI requests only due tiles. There is at most one outstanding request per instance.
/// Requests arriving together share source collection, but each tile keeps its own snapshot.
pub struct Collector {
    sender: Sender<SampleRequest>,
    receiver: Receiver<SampleResult>,
}

impl Collector {
    pub fn start() -> Self {
        let (sender, requests) = mpsc::channel::<SampleRequest>();
        let (results, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut system = System::new();
            let mut disks = Disks::new();
            let mut networks = Networks::new();
            let mut cache = Metrics::default();
            let mut cpu_time: Option<Instant> = None;
            while let Ok(first) = requests.recv() {
                let mut batch = vec![first];
                batch.extend(requests.try_iter());
                let sources: std::collections::BTreeSet<_> = batch
                    .iter()
                    .flat_map(|request| request.sources.iter().copied())
                    .collect();
                let mut network_snapshot = vec![];
                let mut network_time = Instant::now();
                for source in sources {
                    match source {
                        Source::Cpu => {
                            if cpu_time.is_none() {
                                system.refresh_cpu_usage();
                                thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
                            }
                            if cpu_time.is_none_or(|time| {
                                time.elapsed() >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL
                            }) {
                                system.refresh_cpu_usage();
                                cpu_time = Some(Instant::now());
                                cache.cpu =
                                    (!system.cpus().is_empty()).then(|| system.global_cpu_usage());
                                cache.cores = system.cpus().iter().map(|c| c.cpu_usage()).collect();
                            }
                        }
                        Source::Memory => {
                            system.refresh_memory();
                            cache.memory = (system.total_memory() > 0).then(|| MemoryUsage {
                                total: system.total_memory(),
                                available: system.available_memory(),
                                swap_total: system.total_swap(),
                                swap_used: system.used_swap(),
                            });
                        }
                        Source::Network => {
                            networks.refresh(true);
                            network_time = Instant::now();
                            network_snapshot = networks
                                .iter()
                                .map(|(name, data)| NetworkUsage {
                                    name: name.clone(),
                                    received: data.total_received(),
                                    transmitted: data.total_transmitted(),
                                    rates: None,
                                })
                                .collect();
                            network_snapshot.sort_by(|a, b| a.name.cmp(&b.name));
                        }
                        Source::Storage => {
                            disks.refresh(true);
                            cache.disks = disks
                                .list()
                                .iter()
                                .map(|disk| DiskUsage {
                                    mount: disk.mount_point().display().to_string(),
                                    total: disk.total_space(),
                                    available: disk.available_space(),
                                })
                                .collect();
                        }
                        Source::System => {
                            cache.system = Some(SystemInfo {
                                hostname: System::host_name()
                                    .unwrap_or_else(|| "Unknown host".into()),
                                os: System::long_os_version()
                                    .unwrap_or_else(|| std::env::consts::OS.into()),
                                uptime: System::uptime(),
                                logical_cpus: thread::available_parallelism()
                                    .map(usize::from)
                                    .unwrap_or(0),
                            });
                        }
                    }
                }
                for request in batch {
                    // Do not expose unrelated sources refreshed by a faster tile.
                    let mut metrics = Metrics {
                        ready: true,
                        ..Metrics::default()
                    };
                    for source in request.sources {
                        match source {
                            Source::Cpu => {
                                metrics.cpu = cache.cpu;
                                metrics.cores = cache.cores.clone();
                            }
                            Source::Memory => metrics.memory = cache.memory.clone(),
                            Source::Network => {
                                metrics.networks = network_snapshot.clone();
                                metrics.sampled_at = network_time;
                            }
                            Source::Storage => metrics.disks = cache.disks.clone(),
                            Source::System => metrics.system = cache.system.clone(),
                        }
                    }
                    if results.send(SampleResult { request, metrics }).is_err() {
                        return;
                    }
                }
            }
        });
        Self { sender, receiver }
    }

    pub fn request(&self, request: SampleRequest) -> Result<(), mpsc::SendError<SampleRequest>> {
        self.sender.send(request)
    }
    pub fn drain(&self) -> impl Iterator<Item = SampleResult> + '_ {
        self.receiver.try_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn throughput_uses_elapsed_time_and_handles_new_reset_and_removed_interfaces() {
        let mut sampler = NetworkSampler::default();
        let counters = |rx, tx| BTreeMap::from([("eth0".into(), (rx, tx))]);
        assert!(
            sampler.sample(counters(1000, 500), Duration::from_secs(1))[0]
                .rates
                .is_none()
        );
        assert_eq!(
            sampler.sample(counters(3000, 1500), Duration::from_secs(2))[0].rates,
            Some((1000.0, 500.0))
        );
        assert!(
            sampler.sample(counters(1, 1), Duration::from_secs(1))[0]
                .rates
                .is_none()
        );
        assert!(
            sampler
                .sample(BTreeMap::new(), Duration::from_secs(1))
                .is_empty()
        );
        assert!(
            sampler.sample(counters(9000, 9000), Duration::from_secs(1))[0]
                .rates
                .is_none()
        );
        assert!(
            sampler.sample(counters(10000, 10000), Duration::ZERO)[0]
                .rates
                .is_none()
        );
    }
}
