use crate::profiler::FrameSample;
use serde::Serialize;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

const DEFAULT_THRESHOLD_US: u64 = 100_000; // 100ms
const COOLDOWN: Duration = Duration::from_secs(1);
const MAX_LOG_BYTES: u64 = 1_024 * 1_024; // 1 MB

/// Ambient app state captured alongside a performance incident.
#[derive(Clone, Default)]
pub struct PerfContext {
    pub active_sessions: usize,
    pub pane_count: usize,
}

#[derive(Serialize)]
struct HkBreakdown {
    notify_ms: f64,
    activity_ms: f64,
    resize_ms: f64,
    branch_poll_ms: f64,
    file_watch_ms: f64,
    other_ms: f64,
}

#[derive(Serialize)]
struct ContextJson {
    events_in_batch: u32,
    active_sessions: usize,
    panes: usize,
}

#[derive(Serialize)]
struct PerfEntry {
    ts: String,
    total_ms: f64,
    render_ms: f64,
    event_ms: f64,
    event_count: u32,
    housekeeping_ms: f64,
    hk_breakdown: HkBreakdown,
    blame: String,
    context: ContextJson,
}

pub struct PerfLog {
    writer: Option<BufWriter<File>>,
    threshold_us: u64,
    last_write: Instant,
}

impl PerfLog {
    pub fn new() -> Self {
        let path = log_path();
        // Rotate if too large
        if let Ok(meta) = fs::metadata(&path) {
            if meta.len() > MAX_LOG_BYTES {
                let backup = path.with_extension("log.1");
                let _ = fs::rename(&path, backup);
            }
        }

        let writer = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()
            .map(BufWriter::new);

        Self {
            writer,
            threshold_us: DEFAULT_THRESHOLD_US,
            last_write: Instant::now() - COOLDOWN, // allow immediate first write
        }
    }

    /// Check the frame sample and log if it exceeds the threshold.
    pub fn check_and_log(&mut self, sample: &FrameSample, ctx: &PerfContext) {
        if sample.total_us < self.threshold_us {
            return;
        }
        if self.last_write.elapsed() < COOLDOWN {
            return;
        }
        let Some(ref mut writer) = self.writer else {
            return;
        };

        let entry = PerfEntry {
            ts: iso_now(),
            total_ms: us_to_ms(sample.total_us),
            render_ms: us_to_ms(sample.render_us),
            event_ms: us_to_ms(sample.event_us),
            event_count: sample.events_count,
            housekeeping_ms: us_to_ms(sample.housekeeping_us()),
            hk_breakdown: HkBreakdown {
                notify_ms: us_to_ms(sample.notify_us),
                activity_ms: us_to_ms(sample.activity_us),
                resize_ms: us_to_ms(sample.resize_us),
                branch_poll_ms: us_to_ms(sample.branch_poll_us),
                file_watch_ms: us_to_ms(sample.file_watch_us),
                other_ms: us_to_ms(sample.other_hk_us),
            },
            blame: blame(sample),
            context: ContextJson {
                events_in_batch: sample.events_count,
                active_sessions: ctx.active_sessions,
                panes: ctx.pane_count,
            },
        };

        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = writeln!(writer, "{}", json);
            let _ = writer.flush();
        }
        self.last_write = Instant::now();
    }
}

fn log_path() -> PathBuf {
    crate::config::config_dir().join("perf.log")
}

fn us_to_ms(us: u64) -> f64 {
    (us as f64) / 1000.0
}

/// Determine which phase consumed the most time.
fn blame(s: &FrameSample) -> String {
    let phases: [(&str, u64); 8] = [
        ("render", s.render_us),
        ("event_processing", s.event_us),
        ("notify", s.notify_us),
        ("activity", s.activity_us),
        ("resize", s.resize_us),
        ("branch_poll", s.branch_poll_us),
        ("file_watch", s.file_watch_us),
        ("other_hk", s.other_hk_us),
    ];
    phases
        .iter()
        .max_by_key(|(_, v)| *v)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// ISO 8601 timestamp without external dependencies.
fn iso_now() -> String {
    let now = SystemTime::now();
    let dur = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();

    // Simple UTC breakdown
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let h = time_secs / 3600;
    let m = (time_secs % 3600) / 60;
    let s = time_secs % 60;

    // Date from days since epoch (simplified Gregorian)
    let (y, mo, d) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y, mo, d, h, m, s, millis
    )
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = days / 146097;
    let doe = days % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blame_identifies_largest_phase() {
        let mut s = FrameSample::default();
        s.render_us = 10_000;
        s.event_us = 40_000;
        s.branch_poll_us = 5_000;
        assert_eq!(blame(&s), "event_processing");

        s.event_us = 1_000;
        s.render_us = 50_000;
        assert_eq!(blame(&s), "render");
    }

    #[test]
    fn below_threshold_not_logged() {
        let mut log = PerfLog {
            writer: None,
            threshold_us: DEFAULT_THRESHOLD_US,
            last_write: Instant::now() - COOLDOWN,
        };
        let mut s = FrameSample::default();
        s.total_us = 10_000; // 10ms — below threshold
        let ctx = PerfContext::default();
        // Should not panic even with no writer
        log.check_and_log(&s, &ctx);
    }

    #[test]
    fn iso_now_format() {
        let ts = iso_now();
        assert!(ts.ends_with('Z'));
        assert_eq!(ts.len(), 24); // YYYY-MM-DDTHH:MM:SS.mmmZ
    }
}
