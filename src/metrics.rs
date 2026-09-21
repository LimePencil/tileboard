use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Local};
use sysinfo::{Disks, System};

#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    pub cpu: Option<f32>,
    pub cores: Vec<f32>,
    pub disks: Vec<DiskUsage>,
    pub sampled_at: Instant,
    pub now: DateTime<Local>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            cpu: None,
            cores: vec![],
            disks: vec![],
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
            let mut iteration = 0_u64;
            while !worker_stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(1));
                if worker_stop.load(Ordering::Relaxed) {
                    break;
                }
                system.refresh_cpu_usage();
                if iteration.is_multiple_of(10) {
                    disks.refresh(true);
                }
                iteration += 1;
                let snapshot = Metrics {
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
