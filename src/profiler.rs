use crate::perf_log::{PerfContext, PerfLog};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_SAMPLES: usize = 120;

#[derive(Debug, Clone, Default)]
pub struct FrameSample {
    pub render_us: u64,
    pub event_us: u64,
    pub events_count: u32,
    // Housekeeping sub-phases
    pub notify_us: u64,
    pub activity_us: u64,
    pub resize_us: u64,
    pub branch_poll_us: u64,
    pub file_watch_us: u64,
    pub other_hk_us: u64,
    pub total_us: u64,
}

impl FrameSample {
    pub fn housekeeping_us(&self) -> u64 {
        self.notify_us + self.activity_us + self.resize_us + self.branch_poll_us + self.file_watch_us + self.other_hk_us
    }
}

pub struct Profiler {
    pub enabled: bool,
    samples: VecDeque<FrameSample>,
    // In-progress frame being built
    current: FrameSample,
    frame_start: Instant,
    // Worst frame seen (by total_us)
    worst: Option<FrameSample>,
    perf_log: PerfLog,
}

pub struct ProfileSummary {
    pub fps: f64,
    pub frame_avg_ms: f64,
    pub frame_max_ms: f64,
    pub render_avg_ms: f64,
    pub render_max_ms: f64,
    pub event_avg_ms: f64,
    pub events_per_frame: f64,
    pub hk_avg_ms: f64,
    pub hk_max_ms: f64,
    // Sub-phase averages
    pub notify_avg_ms: f64,
    pub activity_avg_ms: f64,
    pub resize_avg_ms: f64,
    pub branch_poll_avg_ms: f64,
    pub file_watch_avg_ms: f64,
    pub other_hk_avg_ms: f64,
    // Worst frame breakdown
    pub worst: Option<FrameSample>,
}

impl Profiler {
    pub fn new() -> Self {
        Self {
            enabled: false,
            samples: VecDeque::with_capacity(MAX_SAMPLES),
            current: FrameSample::default(),
            frame_start: Instant::now(),
            worst: None,
            perf_log: PerfLog::new(),
        }
    }

    pub fn begin_frame(&mut self) {
        // Always track frame start for perf logging, even when overlay is off
        self.frame_start = Instant::now();
        self.current = FrameSample::default();
    }

    pub fn record_render(&mut self, elapsed: Duration) {
        self.current.render_us = elapsed.as_micros() as u64;
    }

    pub fn record_events(&mut self, elapsed: Duration, count: u32) {
        self.current.event_us = elapsed.as_micros() as u64;
        self.current.events_count = count;
    }

    // Housekeeping sub-phase recorders
    pub fn record_notify(&mut self, elapsed: Duration) {
        self.current.notify_us = elapsed.as_micros() as u64;
    }

    pub fn record_activity(&mut self, elapsed: Duration) {
        self.current.activity_us = elapsed.as_micros() as u64;
    }

    pub fn record_resize(&mut self, elapsed: Duration) {
        self.current.resize_us = elapsed.as_micros() as u64;
    }

    pub fn record_branch_poll(&mut self, elapsed: Duration) {
        self.current.branch_poll_us = elapsed.as_micros() as u64;
    }

    pub fn record_file_watch(&mut self, elapsed: Duration) {
        self.current.file_watch_us = elapsed.as_micros() as u64;
    }

    pub fn record_other_hk(&mut self, elapsed: Duration) {
        self.current.other_hk_us = elapsed.as_micros() as u64;
    }

    pub fn finish_frame(&mut self, ctx: &PerfContext) {
        self.current.total_us = self.frame_start.elapsed().as_micros() as u64;

        // Always check perf log, regardless of profiler overlay toggle
        self.perf_log.check_and_log(&self.current, ctx);

        if !self.enabled {
            return;
        }
        let sample = self.current.clone();

        // Track worst frame
        let dominated = self
            .worst
            .as_ref()
            .map_or(true, |w| sample.total_us > w.total_us);
        if dominated {
            self.worst = Some(sample.clone());
        }

        if self.samples.len() >= MAX_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Reset worst-frame tracking (call after reviewing).
    pub fn reset_worst(&mut self) {
        self.worst = None;
    }

    pub fn summary(&self) -> Option<ProfileSummary> {
        if self.samples.is_empty() {
            return None;
        }
        let n = self.samples.len() as f64;
        let mut frame_sum = 0u64;
        let mut frame_max = 0u64;
        let mut render_sum = 0u64;
        let mut render_max = 0u64;
        let mut event_sum = 0u64;
        let mut events_count_sum = 0u64;
        let mut hk_sum = 0u64;
        let mut hk_max = 0u64;
        let mut notify_sum = 0u64;
        let mut activity_sum = 0u64;
        let mut resize_sum = 0u64;
        let mut branch_sum = 0u64;
        let mut fw_sum = 0u64;
        let mut other_sum = 0u64;

        for s in &self.samples {
            frame_sum += s.total_us;
            frame_max = frame_max.max(s.total_us);
            render_sum += s.render_us;
            render_max = render_max.max(s.render_us);
            event_sum += s.event_us;
            events_count_sum += s.events_count as u64;
            let hk = s.housekeeping_us();
            hk_sum += hk;
            hk_max = hk_max.max(hk);
            notify_sum += s.notify_us;
            activity_sum += s.activity_us;
            resize_sum += s.resize_us;
            branch_sum += s.branch_poll_us;
            fw_sum += s.file_watch_us;
            other_sum += s.other_hk_us;
        }

        let frame_avg_ms = (frame_sum as f64 / n) / 1000.0;
        let fps = if frame_avg_ms > 0.0 {
            1000.0 / frame_avg_ms
        } else {
            0.0
        };

        Some(ProfileSummary {
            fps,
            frame_avg_ms,
            frame_max_ms: frame_max as f64 / 1000.0,
            render_avg_ms: render_sum as f64 / n / 1000.0,
            render_max_ms: render_max as f64 / 1000.0,
            event_avg_ms: event_sum as f64 / n / 1000.0,
            events_per_frame: events_count_sum as f64 / n,
            hk_avg_ms: hk_sum as f64 / n / 1000.0,
            hk_max_ms: hk_max as f64 / 1000.0,
            notify_avg_ms: notify_sum as f64 / n / 1000.0,
            activity_avg_ms: activity_sum as f64 / n / 1000.0,
            resize_avg_ms: resize_sum as f64 / n / 1000.0,
            branch_poll_avg_ms: branch_sum as f64 / n / 1000.0,
            file_watch_avg_ms: fw_sum as f64 / n / 1000.0,
            other_hk_avg_ms: other_sum as f64 / n / 1000.0,
            worst: self.worst.clone(),
        })
    }
}
