use std::time::Duration;

use futures_util::future::{select, Either};
use futures_util::StreamExt;
use js_sys::{Date, Math};
use serde_json::Value;
use worker::{Delay, Url, WebSocket, WebsocketEvent};

use crate::ApiError;

use super::types::{NostrEvent, NostrFilter, NostrRawEvent};

const MAX_RELAY_EVENTS: usize = 500;

pub async fn fetch_relay_events(
    relay_url: &str,
    filter: &NostrFilter,
    timeout_ms: u64,
) -> Result<Vec<NostrRawEvent>, ApiError> {
    let url = Url::parse(relay_url)
        .map_err(|err| ApiError::bad_gateway(format!("Invalid relay URL: {err}")))?;
    let ws = WebSocket::connect(url)
        .await
        .map_err(|err| ApiError::bad_gateway(format!("Relay connect failed: {err}")))?;
    let mut events = ws
        .events()
        .map_err(|err| ApiError::internal(format!("Relay event stream failed: {err}")))?;
    ws.accept()
        .map_err(|err| ApiError::internal(format!("Relay accept failed: {err}")))?;

    let sub_id = format!("kb-{}", (Math::random() * 1_000_000_000.0) as u64);
    let req = serde_json::json!(["REQ", sub_id, filter]);
    ws.send_with_str(&req.to_string())
        .map_err(|err| ApiError::internal(format!("Relay send failed: {err}")))?;

    let mut out = Vec::new();
    let start = Date::now();
    let mut done = false;

    while !done {
        if out.len() >= MAX_RELAY_EVENTS {
            break;
        }

        let elapsed = (Date::now() - start).max(0.0) as u64;
        if elapsed >= timeout_ms {
            break;
        }
        let remaining = timeout_ms.saturating_sub(elapsed).max(1);
        let timeout = Delay::from(Duration::from_millis(remaining));
        futures_util::pin_mut!(timeout);
        let next = events.next();
        futures_util::pin_mut!(next);

        match select(next, timeout).await {
            Either::Left((event, _timeout)) => match event {
                Some(Ok(WebsocketEvent::Message(msg))) => {
                    if let Some(text) = msg.text() {
                        if let Some(parsed) = parse_relay_message(&text, &sub_id) {
                            match parsed {
                                RelayMessage::Event(event) => out.push(event),
                                RelayMessage::End => done = true,
                                RelayMessage::Ignore => {}
                            }
                        }
                    }
                }
                Some(Ok(WebsocketEvent::Close(_))) => break,
                Some(Err(err)) => {
                    return Err(ApiError::bad_gateway(format!(
                        "Relay stream error: {err}"
                    )))
                }
                None => break,
            },
            Either::Right((_timeout, _next)) => break,
        }
    }

    let _ = ws.close::<String>(None, None);
    Ok(out)
}

enum RelayMessage {
    Event(NostrRawEvent),
    End,
    Ignore,
}

fn parse_relay_message(text: &str, sub_id: &str) -> Option<RelayMessage> {
    let value: Value = serde_json::from_str(text).ok()?;
    let arr = value.as_array()?;
    let kind = arr.first()?.as_str()?;
    match kind {
        "EVENT" => {
            let sub = arr.get(1)?.as_str()?;
            if sub != sub_id {
                return Some(RelayMessage::Ignore);
            }
            let event_value = arr.get(2)?.clone();
            let event: NostrEvent = serde_json::from_value(event_value.clone()).ok()?;
            let raw_json = serde_json::to_string(&event_value).ok()?;
            Some(RelayMessage::Event(NostrRawEvent { event, raw_json }))
        }
        "EOSE" => {
            let sub = arr.get(1)?.as_str()?;
            if sub == sub_id {
                Some(RelayMessage::End)
            } else {
                Some(RelayMessage::Ignore)
            }
        }
        _ => Some(RelayMessage::Ignore),
    }
}
