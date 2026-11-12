//! Foxglove 프로토콜의 Point Cloud 채널 안내, 구독 및 메시지 전송.

use tokio_tungstenite::tungstenite::Message;

use crate::publisher::protocols::foxglove::encode_foxglove_pointcloud_message;
use crate::publisher::protocols::foxglove::message::{
    Advertise, FOXGLOVE_OP_SUBSCRIBE, FOXGLOVE_OP_UNSUBSCRIBE, FoxgloveOperation, ServerInfo,
    Subscribe, Unsubscribe,
};
use crate::publisher::protocols::{
    PublishTarget, Subscriptions, TextAction, json_message, send_payload,
};
use crate::publisher::{Channel, ChannelRegistry};

pub(super) fn supports_channel(channel: &Channel) -> bool {
    channel.foxglove.is_some()
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

    // 기존 Foxglove 동작에 맞춰 알 수 없거나 형식이 잘못된 메시지는 무시한다.
    Ok(TextAction::Handled(Vec::new()))
}

pub(crate) async fn publish_payload(
    targets: Vec<PublishTarget>,
    timestamp_ns: u64,
    payload: &[u8],
) {
    send_payload(
        targets,
        timestamp_ns,
        payload,
        encode_foxglove_pointcloud_message,
    )
    .await;
}
