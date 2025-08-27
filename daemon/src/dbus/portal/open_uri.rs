use tokio::sync::broadcast;
use zbus::zvariant::Fd;

use crate::EmitEvent;
use std::{
    collections::HashMap,
    fs::File,
    os::fd::{FromRawFd, IntoRawFd, OwnedFd},
    path::Path,
};

#[zbus::proxy(
    interface = "org.freedesktop.portal.OpenURI",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait OpenURI {
    fn open_URI(
        &self,
        parent_window: &str,
        uri: &str,
        options: HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    fn open_file(
        &self,
        parent_window: &str,
        fd: zbus::zvariant::Fd<'_>,
        options: HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    fn open_directory(
        &self,
        parent_window: &str,
        fd: zbus::zvariant::Fd<'_>,
        options: HashMap<&str, zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

enum TargetType {
    Uri,
    File,
    Directory,
}

fn detect_target_type<T>(target: T) -> Option<TargetType>
where
    T: AsRef<str>,
{
    let target = target.as_ref();
    if target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("ftp://")
    {
        return Some(TargetType::Uri);
    }

    let path = if target.starts_with("file://") {
        Path::new(target.trim_start_matches("file://"))
    } else {
        Path::new(target)
    };

    if !path.exists() {
        return None;
    }

    if path.is_dir() {
        Some(TargetType::Directory)
    } else {
        Some(TargetType::File)
    }
}

fn path_to_fd<T>(path: T) -> zbus::Result<Fd<'static>>
where
    T: AsRef<str>,
{
    let clean_path = path.as_ref().trim_start_matches("file://");
    let file = File::open(clean_path)?;

    let raw_fd = file.into_raw_fd();

    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };

    Ok(Fd::from(owned_fd))
}

pub async fn serve(mut receiver: broadcast::Receiver<EmitEvent>) -> zbus::Result<()> {
    let conn = zbus::Connection::session().await?;
    let open_uri = OpenURIProxy::new(&conn).await?;

    tokio::spawn(async move {
        loop {
            if let Ok(EmitEvent::Open { uri, token }) = receiver.recv().await {
                let mut options = HashMap::new();
                if let Some(token) = &token {
                    options.insert("activation_token", zbus::zvariant::Value::new(&**token));
                }

                if let Some(uri_type) = detect_target_type(&uri) {
                    match uri_type {
                        TargetType::Uri => {
                            let _ = open_uri.open_URI("", &uri, options).await;
                        }
                        TargetType::File => {
                            if let Ok(fd) = path_to_fd(&uri) {
                                let _ = open_uri.open_file("", fd, options).await;
                            }
                        }
                        TargetType::Directory => {
                            if let Ok(fd) = path_to_fd(&uri) {
                                let _ = open_uri.open_directory("", fd, options).await;
                            }
                        }
                    }
                }
            }
        }
    });

    Ok(())
}
