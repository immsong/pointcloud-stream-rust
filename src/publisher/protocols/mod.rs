//! WebSocket 하위 프로토콜별 메시지 처리와 공통 전송 타입.

pub mod foxglove;
pub mod wire;

use std::collections::HashMap;

use tokio_tungstenite::tungstenite::Message;

use crate::publisher::{Channel, ChannelId, ChannelRegistry};

pub(crate) type Subscriptions = HashMap<u32, ChannelId>;
pub(crate) type PublishTarget = (tokio::sync::mpsc::Sender<Message>, u32);

pub(crate) enum TextAction {
    Handled(Vec<Message>),
    Forward,
}

pub(crate) fn initial_messages(
    protocol: Option<&str>,
    channels: &[Channel],
) -> Result<Vec<Message>, serde_json::Error> {
    match protocol {
        Some(foxglove::FOXGLOVE_SUBPROTOCOL) => foxglove::handler::initial_messages(channels),
        Some(wire::WIRE_SUBPROTOCOL) => wire::handler::initial_messages(channels),
        _ => Ok(Vec::new()),
    }
}

pub(crate) fn handle_text(
    protocol: Option<&str>,
    text: &str,
    channels: &ChannelRegistry,
    subscriptions: &mut Subscriptions,
) -> Result<TextAction, serde_json::Error> {
    match protocol {
        Some(foxglove::FOXGLOVE_SUBPROTOCOL) => {
            foxglove::handler::handle_text(text, channels, subscriptions)
        }
        Some(wire::WIRE_SUBPROTOCOL) => wire::handler::handle_text(text, channels, subscriptions),
        _ => Ok(TextAction::Forward),
    }
}

pub(crate) fn json_message(value: &impl serde::Serialize) -> Result<Message, serde_json::Error> {
    Ok(Message::Text(serde_json::to_string(value)?.into()))
}

/// 프로토콜별 메시지 인코더를 사용해 구독자의 전송 큐로 payload를 전달한다.
pub(crate) async fn send_payload(
    targets: Vec<PublishTarget>,
    timestamp_ns: u64,
    payload: &[u8],
    encode: fn(u32, u64, &[u8]) -> Vec<u8>,
) {
    for (tx, subscription_id) in targets {
        let message = encode(subscription_id, timestamp_ns, payload);
        let _ = tx.send(Message::Binary(message.into())).await;
    }
}
