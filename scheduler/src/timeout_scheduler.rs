use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time;

pub struct TimeoutScheduler(broadcast::Sender<(u32, String)>);

impl TimeoutScheduler {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(size_of::<u32>() + size_of::<String>());

        Self(tx)
    }

    pub fn timer(&self, id: u32, uuid: String, timeout_millis: u64) -> Timer {
        Timer {
            id,
            uuid,
            sender: self.0.clone(),
            duration: Duration::from_millis(timeout_millis),
        }
    }

    pub fn receiver(&self) -> broadcast::Receiver<(u32, String)> {
        self.0.subscribe()
    }
}

pub struct Timer {
    id: u32,
    uuid: String,
    sender: broadcast::Sender<(u32, String)>,
    duration: Duration,
}

impl Timer {
    pub fn start(self) {
        tokio::spawn(async move {
            let timer = self;

            time::sleep(timer.duration).await;
            _ = timer.sender.send((timer.id, timer.uuid));
        });
    }
}
