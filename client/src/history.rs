use crate::moxnotify::types::{NewNotification, NotificationHints};
use serde::Serialize;
use serde_json::Value;
use zbus::zvariant::Type;

#[derive(Default, PartialEq, Clone, Copy, Type, Serialize)]
pub enum HistoryState {
    #[default]
    Hidden,
    Shown,
}

#[derive(Clone)]
pub struct History {
    state: HistoryState,
    searcher_address: String,
}

#[derive(Serialize)]
struct SearchRequest {
    query: String,
    max_hits: Option<u32>,
    sort_by: Option<String>,
    sort_order: Option<String>,
    start_timestamp: Option<String>,
    end_timestamp: Option<String>,
}

fn parse_iso_timestamp(s: &str) -> Option<i64> {
    let s = s.trim_end_matches('Z');
    if let Some(t_pos) = s.find('T') {
        let date_part = &s[..t_pos];
        let time_part = &s[t_pos + 1..];

        let mut millis = 0u32;
        let time_str = if let Some(dot_pos) = time_part.find('.') {
            let seconds = &time_part[..dot_pos];
            let ms_str = &time_part[dot_pos + 1..];
            millis = ms_str
                .chars()
                .take(3)
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            seconds
        } else {
            time_part
        };

        let date_parts: Vec<&str> = date_part.split('-').collect();
        let time_parts: Vec<&str> = time_str.split(':').collect();

        if date_parts.len() == 3 && time_parts.len() == 3 {
            if let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(min), Ok(sec)) = (
                date_parts[0].parse::<i32>(),
                date_parts[1].parse::<u32>(),
                date_parts[2].parse::<u32>(),
                time_parts[0].parse::<i64>(),
                time_parts[1].parse::<i64>(),
                time_parts[2].parse::<f64>(),
            ) {
                let mut days = 0i64;
                for y in 1970..year {
                    days += if (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0) {
                        366
                    } else {
                        365
                    };
                }
                for m in 1..month {
                    let dim = match m {
                        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                        4 | 6 | 9 | 11 => 30,
                        2 => {
                            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                                29
                            } else {
                                28
                            }
                        }
                        _ => 0,
                    };
                    days += dim as i64;
                }
                days += (day - 1) as i64;
                let total_seconds = days * 86400 + hour * 3600 + min * 60 + sec as i64;
                return Some(total_seconds * 1000 + millis as i64);
            }
        }
    }
    None
}

impl History {
    pub fn new(searcher_address: String) -> Self {
        Self {
            state: HistoryState::default(),
            searcher_address,
        }
    }

    pub fn state(&self) -> HistoryState {
        self.state
    }

    pub fn is_shown(&self) -> bool {
        self.state() == HistoryState::Shown
    }

    pub fn is_hidden(&self) -> bool {
        self.state() == HistoryState::Hidden
    }

    pub fn hide(&mut self) {
        self.state = HistoryState::Hidden;
    }

    pub fn show(&mut self) {
        self.state = HistoryState::Shown;
    }

    pub async fn load_all(&self) -> anyhow::Result<Vec<NewNotification>> {
        let request = SearchRequest {
            query: "*".to_string(),
            max_hits: Some(100),
            sort_by: Some("timestamp".to_string()),
            sort_order: Some("desc".to_string()),
            start_timestamp: None,
            end_timestamp: None,
        };

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/api/search", self.searcher_address))
            .json(&request)
            .send()
            .await?;

        if !resp.status().is_success() {
            anyhow::bail!("Search request failed: {}", resp.status());
        }

        let json_values: Vec<Value> = resp.json().await?;
        log::debug!("Received {} notifications from searcher", json_values.len());
        if !json_values.is_empty() {
            log::debug!("First notification structure: {}", json_values[0]);
        }
        let mut notifications = Vec::new();

        for (idx, json_val) in json_values.iter().enumerate() {
            log::debug!("Processing notification {}: {}", idx, json_val);

            let obj = match json_val {
                Value::Object(map) => map,
                _ => {
                    log::warn!("Skipping non-object value at index {}: {}", idx, json_val);
                    continue;
                }
            };

            let id = match obj.get("id") {
                Some(Value::Array(arr)) => arr
                    .first()
                    .and_then(|v| {
                        if let Value::Number(n) = v {
                            n.as_u64().or_else(|| n.as_i64().map(|i| i as u64))
                        } else {
                            None
                        }
                    })
                    .map(|u| u as u32),
                Some(Value::Number(n)) => {
                    n.as_u64().or_else(|| n.as_i64().map(|i| i as u64)).map(|u| u as u32)
                }
                _ => None,
            };

            let id = match id {
                Some(id) => id,
                None => {
                    log::warn!("Missing or invalid id field at index {}", idx);
                    continue;
                }
            };

            let app_name = obj
                .get("app_name")
                .and_then(|v| {
                    if let Value::Array(arr) = v {
                        arr.first().and_then(|v| v.as_str())
                    } else if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or("unknown")
                .to_string();

            let app_icon = obj
                .get("app_icon")
                .and_then(|v| {
                    if let Value::Array(arr) = v {
                        arr.first().and_then(|v| v.as_str())
                    } else if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .map(|s| s.to_string());

            let summary = obj
                .get("summary")
                .and_then(|v| {
                    if let Value::Array(arr) = v {
                        arr.first().and_then(|v| v.as_str())
                    } else if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or_default()
                .to_string();

            let body = obj
                .get("body")
                .and_then(|v| {
                    if let Value::Array(arr) = v {
                        arr.first().and_then(|v| v.as_str())
                    } else if let Value::String(s) = v {
                        Some(s.as_str())
                    } else {
                        None
                    }
                })
                .unwrap_or_default()
                .to_string();

            let timeout = obj
                .get("timeout")
                .and_then(|v| {
                    if let Value::Array(arr) = v {
                        arr.first().and_then(|v| {
                            if let Value::Number(n) = v {
                                n.as_i64().or_else(|| n.as_u64().map(|u| u as i64))
                            } else {
                                None
                            }
                        })
                    } else if let Value::Number(n) = v {
                        n.as_i64().or_else(|| n.as_u64().map(|u| u as i64))
                    } else {
                        None
                    }
                })
                .unwrap_or(0) as i32;

            let timestamp = obj
                .get("timestamp")
                .and_then(|v| {
                    if let Value::Array(arr) = v {
                        arr.first().and_then(|v| {
                            if let Value::String(s) = v {
                                parse_iso_timestamp(s)
                            } else if let Value::Number(n) = v {
                                n.as_i64().or_else(|| n.as_u64().map(|u| u as i64))
                            } else {
                                None
                            }
                        })
                    } else if let Value::String(s) = v {
                        parse_iso_timestamp(s)
                    } else if let Value::Number(n) = v {
                        n.as_i64().or_else(|| n.as_u64().map(|u| u as i64))
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            let hints = obj
                .get("hints")
                .and_then(|v| {
                    if let Some(s) = v.as_str() {
                        serde_json::from_str::<NotificationHints>(s).ok()
                    } else {
                        serde_json::from_value(v.clone()).ok()
                    }
                })
                .unwrap_or_else(|| NotificationHints {
                    action_icons: false,
                    category: None,
                    value: None,
                    desktop_entry: None,
                    resident: false,
                    sound_file: None,
                    sound_name: None,
                    suppress_sound: false,
                    transient: false,
                    x: 0,
                    y: None,
                    urgency: 1,
                    image: None,
                });

            notifications.push(NewNotification {
                id,
                app_name,
                app_icon,
                summary,
                body,
                timeout,
                actions: vec![],
                hints: Some(hints),
                timestamp,
                uuid: format!("history-{}", id),
            });
        }

        Ok(notifications)
    }
}
