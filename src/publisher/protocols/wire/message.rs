use serde_json::{Map, Value};

use crate::publisher::Channel;
use crate::publisher::protocols::wire::WireSchema;

pub const WIRE_OP_SUBSCRIBE: &str = "subscribe";
pub const WIRE_OP_UNSUBSCRIBE: &str = "unsubscribe";

#[derive(serde::Serialize)]
pub struct ChannelInfo {
    pub id: u32,
    pub topic: String,

    #[serde(flatten)]
    pub schema: WireSchema,

    #[serde(rename = "instanceId")]
    pub instance_id: String,
}

impl From<&Channel> for ChannelInfo {
    fn from(channel: &Channel) -> Self {
        Self {
            id: channel.id.as_u32(),
            topic: channel.topic.clone(),
            schema: channel.schema.clone(),
            instance_id: channel.instance_id.clone(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct ChannelList {
    op: &'static str,
    channels: Vec<ChannelInfo>,
}

impl ChannelList {
    pub fn new(channels: Vec<ChannelInfo>) -> Self {
        Self {
            op: "channels",
            channels,
        }
    }

    pub fn from_channels(channels: &[Channel]) -> Self {
        let channels = channels.iter().map(ChannelInfo::from).collect();

        Self {
            op: "channels",
            channels,
        }
    }
}

#[derive(serde::Deserialize)]
pub struct Subscribe {
    pub op: String,
    pub subscriptions: Vec<Subscription>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: u32,
    pub channel_id: u32,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unsubscribe {
    pub op: String,
    pub subscription_ids: Vec<u32>,
}

#[derive(serde::Deserialize)]
pub struct WireOperation {
    pub op: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireField {
    pub name: String,
    pub offset: u32,
    pub data_type: u8,
    pub count: u32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribedChannel {
    pub id: u32,
    pub channel_id: u32,

    #[serde(flatten)]
    pub schema: WireSchema,

    #[serde(flatten)]
    pub metadata: Map<String, Value>,
}

impl SubscribedChannel {
    pub fn from_channel(subscription_id: u32, channel: &Channel) -> Self {
        Self {
            id: subscription_id,
            channel_id: channel.id.as_u32(),
            schema: channel.schema.clone(),
            metadata: channel.subscription_metadata.clone(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct Subscribed {
    op: &'static str,
    subscriptions: Vec<SubscribedChannel>,
}

impl Subscribed {
    pub fn new(subscriptions: Vec<SubscribedChannel>) -> Self {
        Self {
            op: "subscribed",
            subscriptions,
        }
    }
}
