use std::collections::HashMap;

#[derive(Default)]
struct Notification<'a> {
    app_name: &'a str,
    replaces_id: u32,
    app_icon: &'a str,
    summary: &'a str,
    body: &'a str,
    actions: Box<[&'a str]>,
    hints: HashMap<&'a str, zbus::zvariant::Value<'a>>,
    expire_timeout: i32,
}

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Box<[&str]>,
        hints: HashMap<&str, zbus::zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    #[zbus(signal)]
    async fn action_invoked(&self, nid: u32, action_key: String) -> zbus::Result<()>;
}

async fn emit(notification: Notification<'_>) -> zbus::Result<u32> {
    let conn = zbus::Connection::session().await?;
    let notify = NotificationsProxy::new(&conn).await?;

    notify
        .notify(
            notification.app_name,
            notification.replaces_id,
            notification.app_icon,
            notification.summary,
            notification.body,
            notification.actions,
            notification.hints,
            notification.expire_timeout,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn image_test() {
        let mut hints = HashMap::new();
        hints.insert("image-path", "zen-beta".into());
        let notification = Notification {
            summary: "image-path test",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());

        let notification = Notification {
            summary: "app_icon test",
            app_icon: "zen-beta",
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());

        let mut hints = HashMap::new();
        hints.insert("image-path", "zen-beta".into());
        let notification = Notification {
            summary: "app_icon and image-path test",
            app_icon: "zen-beta",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn replaces_id_test() {
        let notification = Notification {
            summary: "replaces_id test",
            body: "Notification to be replaced",
            ..Default::default()
        };
        let id = emit(notification).await.unwrap();

        tokio::time::sleep(Duration::from_secs(3)).await;

        let notification = Notification {
            replaces_id: id,
            summary: "replaces_id test",
            body: "Replacing notification",
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn expire_test() {
        let mut id = None;
        for i in 0..=5 {
            let notification = Notification {
                summary: "expire test",
                body: &format!("Expires in {} seconds", 5 - i),
                expire_timeout: if i == 5 { 1000 } else { 0 },
                replaces_id: id.unwrap_or(0),
                ..Default::default()
            };
            id = emit(notification).await.ok();

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    #[tokio::test]
    async fn progress_test() {
        let mut hints = HashMap::new();
        hints.insert("value", zbus::zvariant::Value::I32(10));
        let notification = Notification {
            summary: "progress test",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());

        let mut hints = HashMap::new();
        hints.insert("value", zbus::zvariant::Value::I32(0));
        let notification = Notification {
            summary: "progress test",
            body: "Progress value == 0",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());

        let mut hints = HashMap::new();
        hints.insert("value", zbus::zvariant::Value::I32(100));
        let notification = Notification {
            summary: "progress test",
            body: "Progress value == 100",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());

        let mut hints = HashMap::new();
        hints.insert("value", zbus::zvariant::Value::I32(1000));
        let notification = Notification {
            summary: "progress test",
            body: "Progress value > 100",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());

        let mut hints = HashMap::new();
        hints.insert("value", zbus::zvariant::Value::I32(-10));
        let notification = Notification {
            summary: "progress test",
            body: "Negative progress value",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn urgency_low_test() {
        let mut hints = HashMap::new();
        hints.insert("urgency", zbus::zvariant::Value::U8(0));
        hints.insert("value", zbus::zvariant::Value::I32(75));
        let notification = Notification {
            summary: "urgency test",
            body: "Urgency low",
            actions: ["default", "OK"].into(),
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn urgency_normal_test() {
        let mut hints = HashMap::new();
        hints.insert("urgency", zbus::zvariant::Value::U8(1));
        hints.insert("value", zbus::zvariant::Value::I32(75));
        let notification = Notification {
            summary: "urgency test",
            body: "Urgency normal",
            actions: ["default", "OK"].into(),
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn urgency_critical_test() {
        let mut hints = HashMap::new();
        hints.insert("urgency", zbus::zvariant::Value::U8(2));
        hints.insert("value", zbus::zvariant::Value::I32(75));
        let notification = Notification {
            summary: "urgency test",
            body: "Urgency critical",
            actions: ["default", "OK"].into(),
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn everything_test() {
        let mut hints = HashMap::new();
        hints.insert("value", zbus::zvariant::Value::I32(25));
        hints.insert("image-path", "zen-beta".into());

        let body = r#"<u>underline</u>
<i>italic</i>
<b>bold</b>
<a href="https://github.com/unixpariah/moxnotify">github</a>
<img alt="image" href=""/>"#;

        let notification = Notification {
            summary: "everything test",
            body,
            hints,
            actions: ["default", "OK", "cancel", "Cancel", "retry", "Retry"].into(),
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn action_test() {
        let notification = Notification {
            summary: "actions test",
            actions: ["default", "OK"].into(),
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());

        let notification = Notification {
            summary: "actions icon test",
            actions: ["default", "OK"].into(),
            app_icon: "zen-beta",
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn resident_test() {
        let mut hints = HashMap::new();
        hints.insert("resident", zbus::zvariant::Value::Bool(true));
        let notification = Notification {
            summary: "resident test",
            actions: ["default", "OK"].into(),
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn empty_summary_test() {
        let notification = Notification {
            body: "Empty summary test",
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn empty_body_test() {
        let notification = Notification {
            summary: "Empty body test",
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn x_test() {
        let mut hints = HashMap::new();
        hints.insert("x", zbus::zvariant::Value::I32(100));
        let notification = Notification {
            summary: "x test",
            body: "positive value",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());

        let mut hints = HashMap::new();
        hints.insert("x", zbus::zvariant::Value::I32(-100));
        let notification = Notification {
            summary: "x test",
            body: "negative value",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }

    #[tokio::test]
    async fn y_test() {
        let mut hints = HashMap::new();
        hints.insert("y", zbus::zvariant::Value::I32(0));
        let notification = Notification {
            summary: "y test",
            body: "y == 0",
            hints,
            ..Default::default()
        };
        assert!(emit(notification).await.is_ok());
    }
}
