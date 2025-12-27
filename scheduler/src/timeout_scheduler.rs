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

    pub async fn pause(&self, id: u32) {
        if let Some(t) = self.timers.lock().await.get(&id) {
            t.pause();
        }
    }

    pub async fn restart(&self, id: u32) {
        if let Some(t) = self.timers.lock().await.get(&id) {
            t.restart();
        }
    }

    pub async fn restart_all(&self) {
        let timers = self.timers.lock().await;
        for t in timers.values() {
            t.restart();
        }
        let _ = self.global_pause.send(false);
    }

    pub async fn stop(&self, id: u32) {
        if let Some(t) = self.timers.lock().await.remove(&id) {
            t.stop();
        }
    }

    pub fn pause_all(&self) {
        let _ = self.global_pause.send(true);
    }
}

enum TimerCommand {
    Pause,
    Restart,
    Stop,
}

struct TimerHandle {
    cmd: mpsc::UnboundedSender<TimerCommand>,
}

impl TimerHandle {
    fn pause(&self) {
        let _ = self.cmd.send(TimerCommand::Pause);
    }

    fn restart(&self) {
        let _ = self.cmd.send(TimerCommand::Restart);
    }

    fn stop(&self) {
        let _ = self.cmd.send(TimerCommand::Stop);
    }
}

struct Timer;

impl Timer {
    fn spawn(
        id: u32,
        uuid: String,
        duration: Duration,
        sender: broadcast::Sender<(u32, String)>,
        mut cmd_rx: mpsc::UnboundedReceiver<TimerCommand>,
        mut global_pause: watch::Receiver<bool>,
    ) {
        tokio::spawn(async move {
            let initial_duration = duration;
            let mut remaining = duration;
            let mut paused = false;

            loop {
                let start = Instant::now();

                tokio::select! {
                    _ = time::sleep(remaining), if !paused && !*global_pause.borrow() => {
                        let _ = sender.send((id, uuid));
                        break;
                    }

                    Some(cmd) = cmd_rx.recv() => {
                        match cmd {
                            TimerCommand::Pause => paused = true,

                            TimerCommand::Restart => {
                                remaining = initial_duration;
                                paused = false;
                            }

                            TimerCommand::Stop => break,
                        }
                    }

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
