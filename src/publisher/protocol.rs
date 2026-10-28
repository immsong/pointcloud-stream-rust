//! Protocol dispatch and shared transport types. No socket ownership here.
use super::{Channel, ChannelId, ChannelRegistry};
use super::{foxgloves, pointcloud_wire};
use std::collections::HashMap;
use tokio_tungstenite::tungstenite::Message;

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
        Some(foxgloves::FOXGLOVE_SUBPROTOCOL) => foxgloves::handler::initial_messages(channels),
        Some(pointcloud_wire::POINTCLOUD_WIRE_SUBPROTOCOL) => {
            pointcloud_wire::handler::initial_messages(channels)
        }
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
        Some(foxgloves::FOXGLOVE_SUBPROTOCOL) => {
            foxgloves::handler::handle_text(text, channels, subscriptions)
        }
        Some(pointcloud_wire::POINTCLOUD_WIRE_SUBPROTOCOL) => {
            pointcloud_wire::handler::handle_text(text, channels, subscriptions)
        }
        _ => Ok(TextAction::Forward),
    }
}

pub(crate) fn json_message(value: &impl serde::Serialize) -> Result<Message, serde_json::Error> {
    Ok(Message::Text(serde_json::to_string(value)?.into()))
}

/// Shared queue delivery. Encoding is owned by the selected protocol handler.
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
