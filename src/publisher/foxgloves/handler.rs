//! Foxglove compatibility boundary: PointCloud-only discovery, subscriptions and encoding.
use tokio_tungstenite::tungstenite::Message;

use crate::pointcloud::PointCloudFrame;
use crate::publisher::foxgloves::encoder::{
    encode_foxglove_pointcloud_message, encode_foxglove_pointcloud_payload,
};
use crate::publisher::foxgloves::message::{
    Advertise, FOXGLOVE_OP_SUBSCRIBE, FOXGLOVE_OP_UNSUBSCRIBE, FoxgloveOperation, ServerInfo,
    Subscribe, Unsubscribe,
};
use crate::publisher::protocol::{
    PublishTarget, Subscriptions, TextAction, json_message, send_payload,
};
use crate::publisher::{Channel, ChannelKind, ChannelRegistry};

pub(super) fn supports_channel(channel: &Channel) -> bool {
    matches!(&channel.kind, ChannelKind::PointCloud(_))
}

pub(crate) fn initial_messages(channels: &[Channel]) -> Result<Vec<Message>, serde_json::Error> {
    Ok(vec![
        json_message(&ServerInfo::new("pointcloud-stream"))?,
        json_message(&Advertise::from_channels(channels))?,
    ])
}

pub(crate) fn handle_text(
    text: &str,
    channels: &ChannelRegistry,
    subscriptions: &mut Subscriptions,
) -> Result<TextAction, serde_json::Error> {
    if let Ok(operation) = serde_json::from_str::<FoxgloveOperation>(text) {
        match operation.op.as_str() {
            FOXGLOVE_OP_SUBSCRIBE => {
                if let Ok(request) = serde_json::from_str::<Subscribe>(text) {
                    for subscription in request.subscriptions {
                        if let Some(channel) = channels.get_by_raw_id(subscription.channel_id)
                            && supports_channel(&channel)
                        {
                            subscriptions.insert(subscription.id, channel.id);
                        }
                    }
                }
            }
            FOXGLOVE_OP_UNSUBSCRIBE => {
                if let Ok(request) = serde_json::from_str::<Unsubscribe>(text) {
                    for id in request.subscription_ids {
                        subscriptions.remove(&id);
                    }
                }
            }
            _ => {}
        }
    }
    // Preserve legacy Foxglove behavior: unknown/malformed text is ignored.
    Ok(TextAction::Handled(Vec::new()))
}

pub(crate) async fn publish_pointcloud(
    targets: Vec<PublishTarget>,
    frame: &PointCloudFrame,
    point_data: &[u8],
) {
    if targets.is_empty() {
        return;
    }
    // Preserve the existing public publish behavior on encoding failure.
    if let Ok(payload) = encode_foxglove_pointcloud_payload(frame, point_data) {
        send_payload(
            targets,
            frame.timestamp_ns,
            &payload,
            encode_foxglove_pointcloud_message,
        )
        .await;
    }
}
