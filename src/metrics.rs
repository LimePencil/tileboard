use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
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
    /// Bytes per second; None until two valid counter samples are available.
    pub rates: Option<(f64, f64)>,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub hostname: String,
    pub os: String,
    pub uptime: u64,
}

#[derive(Default)]
struct NetworkSampler {
    previous: BTreeMap<String, (u64, u64)>,
}

impl NetworkSampler {
    fn sample(
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

/// System calls run off the event loop. A bounded channel prevents unbounded backlog.
pub struct Collector {
    receiver: Receiver<Metrics>,
    stop: Arc<AtomicBool>,
}

impl Collector {
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        thread::spawn(move || {
            let mut system = System::new();
            system.refresh_cpu_usage();
            let mut disks = Disks::new_with_refreshed_list();
            let mut networks = Networks::new_with_refreshed_list();
            let mut sampler = NetworkSampler::default();
            let counters = |networks: &Networks| {
                networks
                    .iter()
                    .map(|(name, data)| {
                        (
                            name.clone(),
                            (data.total_received(), data.total_transmitted()),
                        )
                    })
                    .collect()
            };
            sampler.sample(counters(&networks), Duration::ZERO);
            let mut network_time = Instant::now();
            let hostname = System::host_name().unwrap_or_else(|| "Unknown host".into());
            let os = System::long_os_version().unwrap_or_else(|| std::env::consts::OS.into());
            let mut iteration = 0_u64;
            while !worker_stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(1));
                if worker_stop.load(Ordering::Relaxed) {
                    break;
                }
                system.refresh_cpu_usage();
                system.refresh_memory();
                networks.refresh(true);
                let now = Instant::now();
                let network_samples =
                    sampler.sample(counters(&networks), now.duration_since(network_time));
                network_time = now;
                if iteration.is_multiple_of(10) {
                    disks.refresh(true);
                }
                iteration += 1;
                let snapshot = Metrics {
                    ready: true,
                    cpu: (!system.cpus().is_empty()).then(|| system.global_cpu_usage()),
                    cores: system.cpus().iter().map(|c| c.cpu_usage()).collect(),
                    disks: disks
                        .list()
                        .iter()
                        .map(|disk| DiskUsage {
                            mount: disk.mount_point().display().to_string(),
                            total: disk.total_space(),
                            available: disk.available_space(),
                        })
                        .collect(),
                    memory: (system.total_memory() > 0).then(|| MemoryUsage {
                        total: system.total_memory(),
                        available: system.available_memory(),
                        swap_total: system.total_swap(),
                        swap_used: system.used_swap(),
                    }),
                    networks: network_samples,
                    system: Some(SystemInfo {
                        hostname: hostname.clone(),
                        os: os.clone(),
                        uptime: System::uptime(),
                    }),
                    sampled_at: Instant::now(),
                    now: Local::now(),
                };
                if let Err(mpsc::TrySendError::Disconnected(_)) = sender.try_send(snapshot) {
                    break;
                }
            }
        });
        Self { receiver, stop }
    }

    pub fn latest(&self) -> Option<Metrics> {
        self.receiver.try_iter().last()
    }
}

impl Drop for Collector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
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
