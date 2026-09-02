pub const FOXGLOVE_OP_SUBSCRIBE: &str = "subscribe";
pub const FOXGLOVE_OP_UNSUBSCRIBE: &str = "unsubscribe";

#[derive(serde::Serialize)]
pub struct ServerInfo {
    op: &'static str,
    name: String,
    capabilities: Vec<&'static str>,
}

impl ServerInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            op: "serverInfo",
            name: name.into(),
            capabilities: std::vec![],
        }
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdvertisedChannel {
    pub id: u32,
    pub topic: String,
    pub encoding: String,
    pub schema_name: String,
    pub schema: String,
}

impl From<&crate::publisher::Channel> for AdvertisedChannel {
    fn from(channel: &crate::publisher::Channel) -> Self {
        Self {
            id: channel.id.as_u32(),
            topic: channel.topic.clone(),
            encoding: "json".to_string(),
            schema_name: "foxglove.PointCloud".to_string(),
            schema: "".to_string(),
        }
    }
}

#[derive(serde::Serialize)]
pub struct Advertise {
    op: &'static str,
    channels: Vec<AdvertisedChannel>,
}

impl Advertise {
    pub fn new(channels: Vec<AdvertisedChannel>) -> Self {
        Self {
            op: "advertise",
            channels,
        }
    }

    pub fn from_channels(channels: &[crate::publisher::Channel]) -> Self {
        let channels = channels.iter().map(AdvertisedChannel::from).collect();

        Self {
            op: "advertise",
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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unsubscribe {
    pub op: String,
    pub subscription_ids: Vec<u32>,
}

#[derive(serde::Deserialize)]
pub struct FoxgloveOperation {
    pub op: String,
}

#[test]
fn server_info_serializes_required_fields() {
    let server_info = ServerInfo::new("pointcloud-stream");

    let json = serde_json::to_value(server_info).unwrap();

    assert_eq!(json["op"], "serverInfo");
    assert_eq!(json["name"], "pointcloud-stream");
    assert_eq!(json["capabilities"], serde_json::json!([]));
}

#[test]
fn advertise_serializes_required_fields() {
    let channel = AdvertisedChannel {
        id: 1,
        topic: "/pointcloud".to_string(),
        encoding: "json".to_string(),
        schema_name: "foxglove.PointCloud".to_string(),
        schema: "".to_string(),
    };

    let advertise = Advertise::new(vec![channel]);

    let json = serde_json::to_value(advertise).unwrap();

    assert_eq!(json["op"], "advertise");
    assert_eq!(json["channels"][0]["id"], 1);
    assert_eq!(json["channels"][0]["topic"], "/pointcloud");
    assert_eq!(json["channels"][0]["encoding"], "json");
    assert_eq!(json["channels"][0]["schemaName"], "foxglove.PointCloud");
    assert_eq!(json["channels"][0]["schema"], "");
}

#[test]
fn subscribe_deserializes_required_fields() {
    let json = r#"
    {
        "op": "subscribe",
        "subscriptions": [
            {
                "id": 0,
                "channelId": 1
            }
        ]
    }
    "#;

    let subscribe: Subscribe = serde_json::from_str(json).unwrap();

    assert_eq!(subscribe.op, "subscribe");
    assert_eq!(subscribe.subscriptions.len(), 1);
    assert_eq!(subscribe.subscriptions[0].id, 0);
    assert_eq!(subscribe.subscriptions[0].channel_id, 1);
}

#[test]
fn unsubscribe_deserializes_required_fields() {
    let json = r#"
    {
        "op": "unsubscribe",
        "subscriptionIds": [0]
    }
    "#;

    let unsubscribe: Unsubscribe = serde_json::from_str(json).unwrap();

    assert_eq!(unsubscribe.op, "unsubscribe");
    assert_eq!(unsubscribe.subscription_ids, vec![0]);
}
