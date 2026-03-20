use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};

use darya::session::pty_session::PtyCallback;
use tui_term::vt100;

/// Create a vt100 parser with a PtyCallback, returning both.
fn make_parser() -> (Arc<RwLock<vt100::Parser<PtyCallback>>>, PtyCallback) {
    let callback = PtyCallback::new();
    let bell_count = callback.bell_count.clone();
    let done_count = callback.done_count.clone();
    let status_text = callback.status_text.clone();
    let parser = vt100::Parser::new_with_callbacks(
        24,
        80,
        0,
        PtyCallback {
            bell_count: bell_count.clone(),
            done_count: done_count.clone(),
            status_text: status_text.clone(),
        },
    );
    (
        Arc::new(RwLock::new(parser)),
        PtyCallback {
            bell_count,
            done_count,
            status_text,
        },
    )
}

#[test]
fn audible_bell_increments_bell_only() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x07");
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 1);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn osc_9_4_0_done_increments_both() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]9;4;0\x1b\\");
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 1);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 1);
}

#[test]
fn osc_9_4_3_indeterminate_does_not_increment() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]9;4;3\x1b\\");
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 0);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn osc_9_4_1_percentage_does_not_increment() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]9;4;1\x1b\\");
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 0);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn osc_9_4_2_error_increments_both() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]9;4;2\x1b\\");
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 1);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 1);
}

#[test]
fn osc_9_message_increments_bell_only() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    // OSC 9;Permission needed ST — generic notification
    p.process(b"\x1b]9;Permission needed\x1b\\");
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 1);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn osc_777_notification_increments_bell_only() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]777;notify;Task;Done\x1b\\");
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 1);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn regular_text_does_not_trigger_callback() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"Hello, world! This is regular terminal output.\r\n");
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 0);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn multiple_sequences_accumulate() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x07"); // BEL → bell only
    p.process(b"\x1b]9;4;0\x1b\\"); // OSC 9;4;0 → both
    p.process(b"\x1b]777;notify;x;y\x1b\\"); // OSC 777 → bell only
    p.process(b"\x1b]9;4;3\x1b\\"); // OSC 9;4;3 (skipped)
    assert_eq!(cb.bell_count.load(Ordering::Relaxed), 3);
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 1);
}

#[test]
fn osc_2_sets_status_text() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]2;Reading src/app.rs\x07");
    assert_eq!(*cb.status_text.read().unwrap(), "Reading src/app.rs");
}

#[test]
fn osc_0_sets_status_text() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]0;Thinking...\x07");
    assert_eq!(*cb.status_text.read().unwrap(), "Thinking...");
}

#[test]
fn non_utf8_title_does_not_panic() {
    let (parser, _cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]2;\xff\xfe\x07");
}

#[test]
fn empty_title_clears_status() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]2;Something\x07");
    assert_eq!(*cb.status_text.read().unwrap(), "Something");
    p.process(b"\x1b]2;\x07");
    assert_eq!(*cb.status_text.read().unwrap(), "");
}

#[test]
fn multiple_titles_last_one_wins() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x1b]2;First\x07");
    p.process(b"\x1b]2;Second\x07");
    p.process(b"\x1b]2;Third\x07");
    assert_eq!(*cb.status_text.read().unwrap(), "Third");
}
