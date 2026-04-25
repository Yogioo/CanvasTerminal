use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessesToUpdate, System};

const DEFAULT_CPU_REFRESH_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_FPS_SMOOTHING: f32 = 0.2;
const MAX_FPS: f32 = 1000.0;

pub(in crate::app) struct PerformanceMetrics {
    system: System,
    pid: Option<Pid>,
    last_cpu_refresh: Option<Instant>,
    fps: f32,
    cpu_usage: Option<f32>,
    visible: bool,
    cpu_refresh_interval: Duration,
    fps_smoothing: f32,
}

impl PerformanceMetrics {
    pub(in crate::app) fn new() -> Self {
        Self {
            system: System::new(),
            pid: sysinfo::get_current_pid().ok(),
            last_cpu_refresh: None,
            fps: 0.0,
            cpu_usage: None,
            visible: false,
            cpu_refresh_interval: DEFAULT_CPU_REFRESH_INTERVAL,
            fps_smoothing: DEFAULT_FPS_SMOOTHING,
        }
    }

    pub(in crate::app) fn update(&mut self, frame_dt: Option<f32>) {
        let now = Instant::now();
        self.update_with_sampler(frame_dt, now, |system, pid| {
            let pid = pid?;
            system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            system.process(pid).map(|process| process.cpu_usage())
        });
    }

    pub(in crate::app) fn fps(&self) -> f32 {
        self.fps
    }

    pub(in crate::app) fn cpu_usage(&self) -> Option<f32> {
        self.cpu_usage
    }

    pub(in crate::app) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(in crate::app) fn toggle_visible(&mut self) {
        self.visible = !self.visible;
    }

    fn update_with_sampler<F>(&mut self, frame_dt: Option<f32>, now: Instant, mut sampler: F)
    where
        F: FnMut(&mut System, Option<Pid>) -> Option<f32>,
    {
        self.update_fps(frame_dt);
        if !self.cpu_refresh_due(now) {
            return;
        }

        self.last_cpu_refresh = Some(now);

        match sampler(&mut self.system, self.pid) {
            Some(value) if value.is_finite() && value >= 0.0 => {
                self.cpu_usage = Some(value);
            }
            Some(_) => {
                // malformed value: keep the last known-good sample
            }
            None => {
                self.cpu_usage = None;
            }
        }
    }

    fn cpu_refresh_due(&self, now: Instant) -> bool {
        match self.last_cpu_refresh {
            None => true,
            Some(last) => now.duration_since(last) >= self.cpu_refresh_interval,
        }
    }

    fn update_fps(&mut self, frame_dt: Option<f32>) {
        let Some(dt) = sanitize_dt(frame_dt) else {
            return;
        };

        let instant_fps = (1.0 / dt).clamp(0.0, MAX_FPS);
        if self.fps <= 0.0 || !self.fps.is_finite() {
            self.fps = instant_fps;
            return;
        }

        self.fps += (instant_fps - self.fps) * self.fps_smoothing;
    }
}

fn sanitize_dt(frame_dt: Option<f32>) -> Option<f32> {
    let dt = frame_dt?;
    if !dt.is_finite() || dt <= 0.0 {
        return None;
    }

    Some(dt.max(1.0 / MAX_FPS))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now_plus(ms: u64) -> Instant {
        Instant::now() + Duration::from_millis(ms)
    }

    #[test]
    fn performance_metrics_throttles_cpu_refreshes() {
        let mut metrics = PerformanceMetrics::new();
        let mut refresh_count = 0;
        let start = Instant::now();

        metrics.update_with_sampler(Some(1.0 / 60.0), start, |_, _| {
            refresh_count += 1;
            Some(11.0)
        });
        metrics.update_with_sampler(Some(1.0 / 60.0), start + Duration::from_millis(500), |_, _| {
            refresh_count += 1;
            Some(22.0)
        });
        metrics.update_with_sampler(Some(1.0 / 60.0), start + Duration::from_millis(1100), |_, _| {
            refresh_count += 1;
            Some(33.0)
        });

        assert_eq!(refresh_count, 2);
        assert_eq!(metrics.cpu_usage(), Some(33.0));
    }

    #[test]
    fn performance_metrics_fallbacks_to_last_valid_cpu_sample_on_malformed_value() {
        let mut metrics = PerformanceMetrics::new();
        let start = Instant::now();

        metrics.update_with_sampler(Some(1.0 / 60.0), start, |_, _| Some(25.0));
        metrics.update_with_sampler(Some(1.0 / 60.0), start + Duration::from_millis(1200), |_, _| {
            Some(f32::NAN)
        });

        assert_eq!(metrics.cpu_usage(), Some(25.0));
    }

    #[test]
    fn performance_metrics_marks_cpu_unavailable_on_sampling_error() {
        let mut metrics = PerformanceMetrics::new();
        let start = Instant::now();

        metrics.update_with_sampler(Some(1.0 / 60.0), start, |_, _| Some(12.0));
        metrics.update_with_sampler(Some(1.0 / 60.0), start + Duration::from_millis(1200), |_, _| {
            None
        });

        assert_eq!(metrics.cpu_usage(), None);
    }

    #[test]
    fn performance_metrics_keeps_fps_finite_under_extreme_frame_intervals() {
        let mut metrics = PerformanceMetrics::new();

        metrics.update_with_sampler(Some(1e-9), now_plus(0), |_, _| Some(10.0));
        metrics.update_with_sampler(Some(10.0), now_plus(1200), |_, _| Some(10.0));

        assert!(metrics.fps().is_finite());
        assert!((0.0..=MAX_FPS).contains(&metrics.fps()));
    }
}
