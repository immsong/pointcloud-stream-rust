mod message;

pub use message::{
    Advertise, Channel, FOXGLOVE_OP_SUBSCRIBE, FOXGLOVE_OP_UNSUBSCRIBE, FoxgloveOperation,
    ServerInfo, Subscribe, Subscription, Unsubscribe,
};

pub const FOXGLOVE_SUBPROTOCOL: &str = "foxglove.websocket.v1";
