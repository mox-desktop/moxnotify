use std::io::{self, Write};

pub enum Event {
    Waiting,
    Focus,
    List,
    DismissAll,
    DismissOne(u32),
    Mute,
    Unmute,
    Inhibit,
    Uninhibit,
    InhibitState,
    ToggleInhibit,
    ToggleMute,
    MuteState,
    SetOutput(Option<String>),
}

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::type_complexity)]
    async fn get_server_information(
        &self,
    ) -> zbus::fdo::Result<(Box<str>, Box<str>, Box<str>, Box<str>)>;
}

#[zbus::proxy(
    interface = "pl.mox.Notify",
    default_service = "pl.mox.Notify",
    default_path = "/pl/mox/Notify"
)]
trait Notify {
    async fn focus(&self) -> zbus::Result<()>;

    async fn list(&self) -> zbus::Result<Vec<String>>;

    async fn dismiss(&self, all: bool, id: u32) -> zbus::Result<()>;

    async fn mute(&self) -> zbus::Result<()>;

    async fn unmute(&self) -> zbus::Result<()>;

    async fn muted(&self) -> zbus::Result<bool>;

    async fn inhibit(&self) -> zbus::Result<()>;

    async fn uninhibit(&self) -> zbus::Result<()>;

    async fn inhibited(&self) -> zbus::Result<bool>;

    async fn waiting(&self) -> zbus::Result<u32>;

    async fn output(&self, all: bool, output: String) -> zbus::Result<()>;
}

pub async fn emit(event: Event) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;

    let notifications = NotificationsProxy::new(&conn).await?;
    let server_information = notifications.get_server_information().await?;
    assert!(
        !(*server_information.0 != *"moxnotify" && *server_information.1 != *"mox"),
        "Unkown notification server"
    );

    let notify = NotifyProxy::new(&conn).await?;
    let mut out = io::stdout().lock();

    match event {
        Event::SetOutput(output) => {
            notify
                .output(output.is_none(), output.unwrap_or("".to_string()))
                .await?
        }
        Event::Focus => notify.focus().await?,
        Event::Waiting => {
            writeln!(out, "{}", notify.waiting().await?)?;
        }
        Event::List => {
            let list = notify.list().await?;
            for item in list {
                writeln!(out, "{item}")?;
            }
        }
        Event::DismissAll => notify.dismiss(true, 0).await?,
        Event::DismissOne(index) => notify.dismiss(false, index).await?,
        Event::Unmute => notify.unmute().await?,
        Event::Mute => notify.mute().await?,
        Event::ToggleMute => {
            if notify.muted().await? {
                notify.unmute().await?;
            } else {
                notify.mute().await?;
            }
        }
        Event::MuteState => {
            if notify.muted().await? {
                writeln!(out, "muted")?;
            } else {
                writeln!(out, "unmuted")?;
            }
        }
        Event::Inhibit => notify.inhibit().await?,
        Event::Uninhibit => notify.uninhibit().await?,
        Event::ToggleInhibit => {
            if notify.inhibited().await? {
                notify.uninhibit().await?;
            } else {
                notify.inhibit().await?;
            }
        }
        Event::InhibitState => {
            if notify.inhibited().await? {
                writeln!(out, "inhibited")?;
            } else {
                writeln!(out, "uninhibited")?;
            }
        }
    }

    Ok(())
}
