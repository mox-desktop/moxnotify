use crate::{EmitEvent, Event, History};
use tokio::sync::broadcast;
use zbus::{fdo::RequestNameFlags, object_server::SignalEmitter};

struct MoxnotifyInterface {
    event_sender: calloop::channel::Sender<Event>,
    emit_receiver: broadcast::Receiver<EmitEvent>,
}

#[zbus::interface(name = "pl.mox.Notify")]
impl MoxnotifyInterface {
    async fn focus(&self) {
        println!("henlo");
        if let Err(e) = self.event_sender.send(Event::FocusSurface) {
            log::error!("{e}");
        }
    }

    async fn dismiss(&self, all: bool, id: u32) {
        if let Err(e) = self.event_sender.send(Event::Dismiss { all, id }) {
            log::error!("{e}");
        }
    }

    async fn waiting(&mut self) -> u32 {
        if let Err(e) = self.event_sender.send(Event::Waiting) {
            log::error!("{e}");
        }

        while let Ok(event) = self.emit_receiver.recv().await {
            if let EmitEvent::Waiting(count) = event {
                return count;
            }
        }

        0
    }

    async fn list(&mut self) -> Vec<String> {
        if let Err(e) = self.event_sender.send(Event::List) {
            log::error!("{e}");
        }

        while let Ok(event) = self.emit_receiver.recv().await {
            if let EmitEvent::List(list) = event {
                return list;
            }
        }

        Vec::new()
    }

    async fn mute(&self) {
        if let Err(e) = self.event_sender.send(Event::Mute) {
            log::error!("{e}");
        }
    }

    async fn unmute(&self) {
        if let Err(e) = self.event_sender.send(Event::Unmute) {
            log::error!("{e}");
        }
    }

    async fn muted(&mut self) -> bool {
        if let Err(e) = self.event_sender.send(Event::GetMuted) {
            log::error!("{e}");
            return false;
        }

        match self.emit_receiver.recv().await {
            Ok(EmitEvent::Muted(muted)) => muted,
            _ => false,
        }
    }

    #[zbus(signal)]
    async fn mute_state_changed(
        signal_emitter: &SignalEmitter<'_>,
        muted: bool,
    ) -> zbus::Result<()>;

    async fn show_history(&self) {
        if let Err(e) = self.event_sender.send(Event::ShowHistory) {
            log::error!("{e}");
        }
    }

    async fn hide_history(&self) {
        if let Err(e) = self.event_sender.send(Event::HideHistory) {
            log::error!("{e}");
        }
    }

    async fn history(&mut self) -> History {
        if let Err(e) = self.event_sender.send(Event::GetHistory) {
            log::error!("{e}");
            return History::Hidden;
        }

        match self.emit_receiver.recv().await {
            Ok(EmitEvent::HistoryState(history)) => history,
            _ => History::Hidden,
        }
    }

    #[zbus(signal)]
    async fn history_state_changed(
        signal_emitter: &SignalEmitter<'_>,
        history: History,
    ) -> zbus::Result<()>;

    async fn inhibit(&self) {
        if let Err(e) = self.event_sender.send(Event::Inhibit) {
            log::error!("{e}");
        }
    }

    async fn uninhibit(&self) {
        if let Err(e) = self.event_sender.send(Event::Uninhibit) {
            log::error!("{e}");
        }
    }

    async fn inhibited(&mut self) -> bool {
        if let Err(e) = self.event_sender.send(Event::GetInhibited) {
            log::error!("{e}");
            return false;
        }

        match self.emit_receiver.recv().await {
            Ok(EmitEvent::Inhibited(inhibited)) => inhibited,
            _ => false,
        }
    }

    #[zbus(signal)]
    async fn inhibit_changed(
        signal_emitter: &SignalEmitter<'_>,
        inhibited: bool,
    ) -> zbus::Result<()>;
}

pub async fn serve(
    event_sender: calloop::channel::Sender<Event>,
    mut emit_receiver: broadcast::Receiver<EmitEvent>,
) -> zbus::Result<()> {
    let server = MoxnotifyInterface {
        event_sender,
        emit_receiver: emit_receiver.resubscribe(),
    };

    let conn = zbus::connection::Builder::session()?
        .serve_at("/pl/mox/Notify", server)?
        .build()
        .await?;

    conn.request_name_with_flags(
        "pl.mox.Notify",
        // If in release mode, exit if well-known name is already taken
        #[cfg(not(debug_assertions))]
        (RequestNameFlags::DoNotQueue | RequestNameFlags::AllowReplacement),
        // If in debug profile, replace already existing daemon
        #[cfg(debug_assertions)]
        RequestNameFlags::ReplaceExisting.into(),
    )
    .await?;

    let iface = conn
        .object_server()
        .interface::<_, MoxnotifyInterface>("/pl/mox/Notify")
        .await?;

    tokio::spawn(async move {
        loop {
            match emit_receiver.recv().await {
                Ok(EmitEvent::MuteStateChanged(muted)) => {
                    if let Err(e) =
                        MoxnotifyInterfaceSignals::mute_state_changed(iface.signal_emitter(), muted)
                            .await
                    {
                        log::error!("{e}");
                    }
                }
                Ok(EmitEvent::HistoryStateChanged(history)) => {
                    if let Err(e) = MoxnotifyInterfaceSignals::history_state_changed(
                        iface.signal_emitter(),
                        history,
                    )
                    .await
                    {
                        log::error!("{e}");
                    }
                }
                Ok(EmitEvent::InhibitStateChanged(inhibited)) => {
                    if let Err(e) = MoxnotifyInterfaceSignals::inhibit_changed(
                        iface.signal_emitter(),
                        inhibited,
                    )
                    .await
                    {
                        log::error!("{e}");
                    }
                }
                Err(e) => log::error!("{e}"),
                _ => {}
            }
        }
    });

    Ok(())
}
