use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, broadcast, mpsc, watch},
    time::{self, Instant},
};

pub struct TimeoutScheduler {
    sender: broadcast::Sender<(u32, String)>,
    global_pause: watch::Sender<bool>,
    timers: Arc<Mutex<HashMap<u32, TimerHandle>>>,
}

impl TimeoutScheduler {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(32);
        let (global_pause, _) = watch::channel(false);

        Self {
            sender,
            global_pause,
            timers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn start_timer(&self, id: u32, uuid: String, duration: Duration) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        Timer::spawn(
            id,
            uuid,
            duration,
            self.sender.clone(),
            cmd_rx,
            self.global_pause.subscribe(),
        );

        self.timers
            .lock()
            .await
            .insert(id, TimerHandle { cmd: cmd_tx });
    }

    pub fn receiver(&self) -> broadcast::Receiver<(u32, String)> {
        self.sender.subscribe()
    }

    pub async fn stop(&self, id: u32) {
        if let Some(t) = self.timers.lock().await.remove(&id) {
            t.stop();
        }
    }
}

struct TimerHandle {
    cmd: mpsc::UnboundedSender<()>,
}

impl TimerHandle {
    fn stop(&self) {
        let _ = self.cmd.send(());
    }
}

struct Timer;

impl Timer {
    fn spawn(
        id: u32,
        uuid: String,
        duration: Duration,
        sender: broadcast::Sender<(u32, String)>,
        mut cmd_rx: mpsc::UnboundedReceiver<()>,
        mut global_pause: watch::Receiver<bool>,
    ) {
        tokio::spawn(async move {
            let mut remaining = duration;
            let mut paused = false;

            loop {
                let start = Instant::now();

                tokio::select! {
                    _ = time::sleep(remaining), if !paused && !*global_pause.borrow() => {
                        let _ = sender.send((id, uuid));
                        break;
                    }

                    _ = cmd_rx.recv() => break,

                    _ = global_pause.changed() => {
                        paused = *global_pause.borrow();
                    }
                }

                if !paused {
                    remaining = remaining.saturating_sub(start.elapsed());
                }
            }
        });
    }
}
