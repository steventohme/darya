use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};

use darya::session::pty_session::PtyCallback;
use tui_term::vt100;

/// Create a vt100 parser with a PtyCallback, returning both.
fn make_parser() -> (Arc<RwLock<vt100::Parser<PtyCallback>>>, PtyCallback) {
    let callback = PtyCallback::new();
    let done_count = callback.done_count.clone();
    let parser = vt100::Parser::new_with_callbacks(24, 80, 0, PtyCallback {
        done_count: done_count.clone(),
    });
    // Return the original callback's done_count via a new PtyCallback that shares the Arc
    (
        Arc::new(RwLock::new(parser)),
        PtyCallback { done_count },
    )
}

#[test]
fn audible_bell_increments_done_count() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    // BEL character
    p.process(b"\x07");
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 1);
}

#[test]
fn osc_9_4_0_done_increments() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    // OSC 9;4;0 ST (progress done)
    p.process(b"\x1b]9;4;0\x1b\\");
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 1);
}

#[test]
fn osc_9_4_3_indeterminate_does_not_increment() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    // OSC 9;4;3 ST (indeterminate progress — skip)
    p.process(b"\x1b]9;4;3\x1b\\");
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn osc_9_4_1_percentage_does_not_increment() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    // OSC 9;4;1 ST (percentage progress — skip)
    p.process(b"\x1b]9;4;1\x1b\\");
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn osc_9_4_2_error_increments() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    // OSC 9;4;2 ST (progress error — attention-worthy)
    p.process(b"\x1b]9;4;2\x1b\\");
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 1);
}

#[test]
fn osc_777_notification_increments() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    // OSC 777;notify;title;body ST
    p.process(b"\x1b]777;notify;Task;Done\x1b\\");
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 1);
}

#[test]
fn regular_text_does_not_trigger_callback() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"Hello, world! This is regular terminal output.\r\n");
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 0);
}

#[test]
fn multiple_sequences_accumulate() {
    let (parser, cb) = make_parser();
    let mut p = parser.write().unwrap();
    p.process(b"\x07"); // BEL
    p.process(b"\x1b]9;4;0\x1b\\"); // OSC 9;4;0
    p.process(b"\x1b]777;notify;x;y\x1b\\"); // OSC 777
    p.process(b"\x1b]9;4;3\x1b\\"); // OSC 9;4;3 (skipped)
    assert_eq!(cb.done_count.load(Ordering::Relaxed), 3);
}
