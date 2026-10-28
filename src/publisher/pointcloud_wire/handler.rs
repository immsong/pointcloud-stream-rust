//! Existing Wire behavior, isolated from WebSocket connection management.
use super::*;
use crate::publisher::protocol::{
    PublishTarget, Subscriptions, TextAction, json_message, send_payload,
};
use crate::publisher::{Channel, ChannelRegistry};
use tokio_tungstenite::tungstenite::Message;

pub(crate) fn initial_messages(channels: &[Channel]) -> Result<Vec<Message>, serde_json::Error> {
    Ok(vec![json_message(&ChannelList::from_channels(channels))?])
}

pub(crate) fn handle_text(
    text: &str,
    channels: &ChannelRegistry,
    subscriptions: &mut Subscriptions,
) -> Result<TextAction, serde_json::Error> {
    if let Ok(operation) = serde_json::from_str::<WireOperation>(text) {
        match operation.op.as_str() {
            WIRE_OP_SUBSCRIBE => {
                if let Ok(request) = serde_json::from_str::<Subscribe>(text) {
                    let mut accepted = Vec::new();
                    for subscription in request.subscriptions {
                        if let Some(channel) = channels.get_by_raw_id(subscription.channel_id) {
                            subscriptions.insert(subscription.id, channel.id);
                            accepted
                                .push(SubscribedChannel::from_channel(subscription.id, &channel));
                        }
                    }
                    return Ok(TextAction::Handled(vec![json_message(&Subscribed::new(
                        accepted,
                    ))?]));
                }
                return Ok(TextAction::Handled(Vec::new()));
            }
            WIRE_OP_UNSUBSCRIBE => {
                if let Ok(request) = serde_json::from_str::<Unsubscribe>(text) {
                    for id in request.subscription_ids {
                        subscriptions.remove(&id);
                    }
                }
                return Ok(TextAction::Handled(Vec::new()));
            }
            _ => {}
        }
    }
    Ok(TextAction::Forward)
}

pub(crate) async fn publish_pointcloud(
    targets: Vec<PublishTarget>,
    timestamp_ns: u64,
    data: &[u8],
) {
    send_payload(targets, timestamp_ns, data, encode_wire_pointcloud_message).await;
}
