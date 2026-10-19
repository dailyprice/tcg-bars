//! Live updates. Every write publishes an event, and browsers subscribe to
//! `GET /api/events` (server-sent events) so a bar someone adds or rates shows
//! up on everyone's map without a reload.

use serde::Serialize;
use tokio::sync::broadcast;

use crate::db::BarFeature;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BarEvent {
    BarCreated { bar: BarFeature },
    BarMoved { bar: BarFeature },
    BarRated { bar: BarFeature },
}

impl BarEvent {
    pub fn name(&self) -> &'static str {
        match self {
            BarEvent::BarCreated { .. } => "bar_created",
            BarEvent::BarMoved { .. } => "bar_moved",
            BarEvent::BarRated { .. } => "bar_rated",
        }
    }
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<BarEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn publish(&self, event: BarEvent) {
        // An error only means nobody is listening right now.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BarEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
