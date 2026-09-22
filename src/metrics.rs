use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use crate::integrations::ExternalData;
use chrono::{DateTime, Local};
use sysinfo::{Components, Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, System};

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
pub struct ProcessUsage {
    pub pid: u32,
    pub name: String,
    pub cpu: Option<f32>,
    pub memory: u64,
}
#[derive(Debug, Clone)]
pub struct Temperature {
    pub label: String,
    pub celsius: f32,
    pub critical: Option<f32>,
}
#[derive(Debug, Clone)]
pub struct BatteryUsage {
    pub percent: f32,
    pub state: String,
    pub health: f32,
    pub remaining_minutes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    pub processes: Vec<ProcessUsage>,
    pub temperatures: Vec<Temperature>,
    pub batteries: Option<Result<Vec<BatteryUsage>, String>>,
    pub external: Option<Result<ExternalData, String>>,
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
            processes: vec![],
            temperatures: vec![],
            batteries: None,
            external: None,
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
    Processes,
    Temperature,
    Battery,
    Git,
    Service,
    Weather,
    Usage,
}

pub type TileKey = (String, String, String);

#[derive(Debug, Clone)]
pub struct SampleRequest {
    pub options: toml::Table,
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
    external: BTreeMap<Source, Sender<SampleRequest>>,
    sender: Sender<SampleRequest>,
    receiver: Receiver<SampleResult>,
}

impl Collector {
    pub fn start() -> Self {
        let (sender, requests) = mpsc::channel::<SampleRequest>();
        let (results, receiver) = mpsc::channel();
        let mut external = BTreeMap::new();
        for source in [
            Source::Git,
            Source::Service,
            Source::Weather,
            Source::Usage,
            Source::Battery,
        ] {
            let (sender, requests) = mpsc::channel::<SampleRequest>();
            external.insert(source, sender);
            let results = results.clone();
            thread::spawn(move || {
                let client = crate::integrations::client();
                while let Ok(request) = requests.recv() {
                    let mut metrics = Metrics {
                        ready: true,
                        ..Metrics::default()
                    };
                    if source == Source::Battery {
                        metrics.batteries = Some(read_batteries());
                    } else {
                        metrics.external = Some(match &client {
                            Ok(client) => {
                                crate::integrations::collect(source, &request.options, client)
                                    .map_err(|e| format!("{e:#}"))
                            }
                            Err(_) => Err("Cannot initialize HTTP client".into()),
                        });
                    }
                    if results.send(SampleResult { request, metrics }).is_err() {
                        break;
                    }
                }
            });
        }
        thread::spawn(move || {
            let mut system = System::new();
            let mut processes = System::new();
            let mut process_time: Option<Instant> = None;
            let mut components = Components::new();
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
                        Source::Processes => {
                            if process_time.is_none_or(|time| {
                                time.elapsed() >= sysinfo::MINIMUM_CPU_UPDATE_INTERVAL
                            }) {
                                let warmed = process_time.is_some();
                                processes.refresh_processes_specifics(
                                    ProcessesToUpdate::All,
                                    true,
                                    ProcessRefreshKind::nothing()
                                        .with_cpu()
                                        .with_memory()
                                        .without_tasks(),
                                );
                                process_time = Some(Instant::now());
                                cache.processes = processes
                                    .processes()
                                    .iter()
                                    .map(|(pid, p)| ProcessUsage {
                                        pid: pid.as_u32(),
                                        name: p.name().to_string_lossy().into_owned(),
                                        cpu: warmed.then(|| p.cpu_usage()),
                                        memory: p.memory(),
                                    })
                                    .collect();
                            }
                        }
                        Source::Temperature => {
                            components.refresh(true);
                            cache.temperatures = components
                                .iter()
                                .filter_map(|c| {
                                    c.temperature().filter(|v| v.is_finite()).map(|celsius| {
                                        Temperature {
                                            label: c.label().into(),
                                            celsius,
                                            critical: c.critical(),
                                        }
                                    })
                                })
                                .collect();
                        }
                        Source::Battery
                        | Source::Git
                        | Source::Service
                        | Source::Weather
                        | Source::Usage => {}
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
                            Source::Processes => metrics.processes = cache.processes.clone(),
                            Source::Temperature => {
                                metrics.temperatures = cache.temperatures.clone()
                            }
                            Source::Battery
                            | Source::Git
                            | Source::Service
                            | Source::Weather
                            | Source::Usage => {}
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
        Self {
            sender,
            receiver,
            external,
        }
    }

    pub fn request(&self, request: SampleRequest) -> anyhow::Result<()> {
        if let [source] = request.sources
            && let Some(sender) = self.external.get(source)
        {
            return sender
                .send(request)
                .map_err(|_| anyhow::anyhow!("Metrics worker stopped"));
        }
        self.sender
            .send(request)
            .map_err(|_| anyhow::anyhow!("Metrics worker stopped"))
    }
    pub fn drain(&self) -> impl Iterator<Item = SampleResult> + '_ {
        self.receiver.try_iter()
    }
}

fn read_batteries() -> Result<Vec<BatteryUsage>, String> {
    use starship_battery::{
        Manager,
        units::{ratio::percent, time::minute},
    };
    let read = || -> anyhow::Result<Vec<BatteryUsage>> {
        let manager = Manager::new()?;
        let mut batteries = vec![];
        for battery in manager.batteries()? {
            let battery = battery?;
            batteries.push(BatteryUsage {
                percent: battery.state_of_charge().get::<percent>(),
                state: format!("{:?}", battery.state()),
                health: battery.state_of_health().get::<percent>(),
                remaining_minutes: battery
                    .time_to_full()
                    .or_else(|| battery.time_to_empty())
                    .map(|time| time.get::<minute>().max(0.0) as u64),
            });
        }
        Ok(batteries)
    };
    read().map_err(|_| "Battery information unavailable".into())
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
