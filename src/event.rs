use crossterm::event::{Event, EventStream, KeyEvent};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::TICK_RATE_MS;

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppEvent {
    Key(KeyEvent),
    Resize(u16, u16),
    PtyOutput { session_id: String },
    SessionBell { session_id: String },
    SessionExited { session_id: String },
    Tick,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}

/// Create an event handler and return both the handler and a sender for PTY events.
pub fn create_event_handler() -> (EventHandler, mpsc::UnboundedSender<AppEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();

    // Crossterm event reader task
    let event_tx = tx.clone();
    tokio::spawn(async move {
        let mut reader = EventStream::new();
        loop {
            match reader.next().await {
                Some(Ok(event)) => {
                    let app_event = match event {
                        Event::Key(key) => Some(AppEvent::Key(key)),
                        Event::Resize(w, h) => Some(AppEvent::Resize(w, h)),
                        _ => None,
                    };
                    if let Some(e) = app_event {
                        if event_tx.send(e).is_err() {
                            break;
                        }
                    }
                }
                Some(Err(_)) => break,
                None => break,
            }
        }
    });

    // Tick timer task
    let tick_tx = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_RATE_MS));
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    (EventHandler { rx }, tx)
}
