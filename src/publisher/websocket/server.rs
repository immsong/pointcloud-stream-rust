use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};

use crate::pointcloud::{PointCloudFrame, PointCloudLayout, pack_point_data};
use crate::publisher::foxgloves::{
    Advertise, FOXGLOVE_OP_SUBSCRIBE, FOXGLOVE_OP_UNSUBSCRIBE, FOXGLOVE_SUBPROTOCOL,
    FoxgloveOperation, ServerInfo, Subscribe as FoxgloveSubscribe,
    Unsubscribe as FoxgloveUnsubscribe, encode_foxglove_pointcloud_message,
    encode_foxglove_pointcloud_payload,
};
use crate::publisher::pointcloud_wire::{
    ChannelList, POINTCLOUD_WIRE_SUBPROTOCOL, Subscribe as WireSubscribe, Subscribed,
    SubscribedChannel, Unsubscribe as WireUnsubscribe, WIRE_OP_SUBSCRIBE, WIRE_OP_UNSUBSCRIBE,
    WireOperation, encode_wire_pointcloud_message,
};
use crate::publisher::websocket::WebsocketEvent;
use crate::publisher::{ChannelId, ChannelRegistry};

const SUPPORTED_SUBPROTOCOLS: &[&str] = &[FOXGLOVE_SUBPROTOCOL, POINTCLOUD_WIRE_SUBPROTOCOL];

#[derive(Debug)]
struct ClientState {
    tx: tokio::sync::mpsc::Sender<tokio_tungstenite::tungstenite::Message>,
    subprotocol: Option<&'static str>,
    subscriptions: HashMap<u32, ChannelId>,
}

#[derive(Clone)]
pub struct WebsocketServer {
    address: String,
    clients: Arc<tokio::sync::Mutex<HashMap<std::net::SocketAddr, ClientState>>>,
    channels: ChannelRegistry,
}

impl WebsocketServer {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            channels: ChannelRegistry::new(),
        }
    }

    pub fn register_channel(
        &mut self,
        topic: impl Into<String>,
        layout: PointCloudLayout,
    ) -> ChannelId {
        self.channels.register(topic, layout)
    }

    pub async fn run(
        &self,
        ready: tokio::sync::oneshot::Sender<std::net::SocketAddr>,
        event_tx: tokio::sync::mpsc::Sender<WebsocketEvent>,
    ) -> std::io::Result<()> {
        let listen = tokio::net::TcpListener::bind(&self.address).await?;
        let local_addr = listen.local_addr()?;

        // listener 생성 완료 통지.
        let _ = ready.send(local_addr);

        loop {
            // 새로운 클라이언트 연결 대기.
            let (stream, addr) = listen.accept().await?;

            let clients = self.clients.clone();
            let event_tx = event_tx.clone();
            let channels = self.channels.clone();

            tokio::spawn(async move {
                Self::handle_connection(stream, addr, clients, event_tx, channels).await;
            });
        }
    }

    /// 웹소켓 클라이언트 연결을 처리한다.
    async fn handle_connection(
        stream: tokio::net::TcpStream,
        addr: std::net::SocketAddr,
        clients: Arc<tokio::sync::Mutex<HashMap<std::net::SocketAddr, ClientState>>>,
        event_tx: tokio::sync::mpsc::Sender<WebsocketEvent>,
        channels: ChannelRegistry,
    ) {
        //  websocket handshake
        let mut client_subprotocol = None;
        let callback =
            |request: &tokio_tungstenite::tungstenite::handshake::server::Request,
             mut response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                if let Some(subprotocol) = Self::select_subprotocol(request) {
                    client_subprotocol = Some(subprotocol);
                    response.headers_mut().insert(
                        tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL,
                        tokio_tungstenite::tungstenite::http::HeaderValue::from_static(subprotocol),
                    );
                }
                Ok(response)
            };

        let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, callback).await {
            Ok(ws_stream) => ws_stream,
            Err(_e) => {
                return;
            }
        };

        // 웹소켓 스트림을 읽기와 쓰기로 분리.
        let (mut ws_writer, mut ws_reader) = ws_stream.split();
        // 다른 task에서 WebSocket writer로 메시지를 전달하기 위한 channel.
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<tokio_tungstenite::tungstenite::Message>(32);

        let client_state = ClientState {
            tx: tx.clone(),
            subprotocol: client_subprotocol,
            subscriptions: HashMap::new(),
        };
        clients.lock().await.insert(addr, client_state);

        // WebSocket write를 전담하는 task.
        //
        // 다른 로직에서는 ws_writer에 직접 접근하지 않고
        // mpsc sender(tx)를 통해 전송할 메시지를 전달한다.
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if ws_writer.send(msg).await.is_err() {
                    return;
                }
            }
        });

        match client_subprotocol {
            Some(POINTCLOUD_WIRE_SUBPROTOCOL) => {
                let channel_list = ChannelList::from_channels(channels.channels());
                let json = match serde_json::to_string(&channel_list) {
                    Ok(json) => json,
                    Err(_) => {
                        clients.lock().await.remove(&addr);
                        return;
                    }
                };

                let result = tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
                    .await;

                if result.is_err() {
                    clients.lock().await.remove(&addr);
                    return;
                }
            }
            Some(FOXGLOVE_SUBPROTOCOL) => {
                let server_info = ServerInfo::new("pointcloud-stream");

                let json = match serde_json::to_string(&server_info) {
                    Ok(json) => json,
                    Err(_) => {
                        clients.lock().await.remove(&addr);
                        return;
                    }
                };

                if tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
                    .await
                    .is_err()
                {
                    clients.lock().await.remove(&addr);
                    return;
                }

                let advertise = Advertise::from_channels(channels.channels());
                let json = match serde_json::to_string(&advertise) {
                    Ok(json) => json,
                    Err(_) => {
                        clients.lock().await.remove(&addr);
                        return;
                    }
                };

                if tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(json.into()))
                    .await
                    .is_err()
                {
                    clients.lock().await.remove(&addr);
                    return;
                }
            }
            _ => {}
        }

        // client 등록 후 연결 완료 이벤트를 전송.
        let _ = event_tx
            .send(WebsocketEvent::Connected {
                addr,
                subprotocol: client_subprotocol,
            })
            .await;

        loop {
            // client로부터 다음 WebSocket 메시지를 기다린다.
            let some_message = ws_reader.next().await;
            match some_message {
                Some(Ok(msg)) => match msg {
                    tokio_tungstenite::tungstenite::Message::Text(text) => match client_subprotocol
                    {
                        Some(FOXGLOVE_SUBPROTOCOL) => {
                            if let Ok(operation) = serde_json::from_str::<FoxgloveOperation>(&text)
                            {
                                match operation.op.as_str() {
                                    FOXGLOVE_OP_SUBSCRIBE => {
                                        if let Ok(subscribe) =
                                            serde_json::from_str::<FoxgloveSubscribe>(&text)
                                        {
                                            let mut clients = clients.lock().await;
                                            let client_state = clients.get_mut(&addr);
                                            if let Some(client_state) = client_state {
                                                for subscription in subscribe.subscriptions {
                                                    if let Some(channel) = channels
                                                        .get_by_raw_id(subscription.channel_id)
                                                    {
                                                        client_state
                                                            .subscriptions
                                                            .insert(subscription.id, channel.id);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    FOXGLOVE_OP_UNSUBSCRIBE => {
                                        if let Ok(unsubscribe) =
                                            serde_json::from_str::<FoxgloveUnsubscribe>(&text)
                                        {
                                            let mut clients = clients.lock().await;
                                            let client_state = clients.get_mut(&addr);
                                            if let Some(client_state) = client_state {
                                                for subscription_id in unsubscribe.subscription_ids
                                                {
                                                    client_state
                                                        .subscriptions
                                                        .remove(&subscription_id);
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Some(POINTCLOUD_WIRE_SUBPROTOCOL) => {
                            if let Ok(operation) = serde_json::from_str::<WireOperation>(&text) {
                                match operation.op.as_str() {
                                    WIRE_OP_SUBSCRIBE => {
                                        if let Ok(subscribe) =
                                            serde_json::from_str::<WireSubscribe>(&text)
                                        {
                                            let mut subscribed_channels = vec![];
                                            {
                                                let mut clients = clients.lock().await;
                                                let client_state = clients.get_mut(&addr);
                                                if let Some(client_state) = client_state {
                                                    for subscription in subscribe.subscriptions {
                                                        if let Some(channel) = channels
                                                            .get_by_raw_id(subscription.channel_id)
                                                        {
                                                            client_state.subscriptions.insert(
                                                                subscription.id,
                                                                channel.id,
                                                            );

                                                            subscribed_channels.push(
                                                                SubscribedChannel::from_channel(
                                                                    subscription.id,
                                                                    channel,
                                                                ),
                                                            );
                                                        }
                                                    }
                                                }
                                            }

                                            let subscribed = Subscribed::new(subscribed_channels);

                                            let json = match serde_json::to_string(&subscribed) {
                                                Ok(json) => json,
                                                Err(_) => {
                                                    continue;
                                                }
                                            };

                                            if tx
                                                .send(
                                                    tokio_tungstenite::tungstenite::Message::Text(
                                                        json.into(),
                                                    ),
                                                )
                                                .await
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                    }
                                    WIRE_OP_UNSUBSCRIBE => {
                                        if let Ok(unsubscribe) =
                                            serde_json::from_str::<WireUnsubscribe>(&text)
                                        {
                                            let mut clients = clients.lock().await;
                                            let client_state = clients.get_mut(&addr);
                                            if let Some(client_state) = client_state {
                                                for subscription_id in unsubscribe.subscription_ids
                                                {
                                                    client_state
                                                        .subscriptions
                                                        .remove(&subscription_id);
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {
                            let _ = event_tx
                                .send(WebsocketEvent::Message {
                                    addr,
                                    message: tokio_tungstenite::tungstenite::Message::Text(text),
                                })
                                .await;
                        }
                    },
                    tokio_tungstenite::tungstenite::Message::Binary(bin) => {
                        if event_tx
                            .send(WebsocketEvent::Message {
                                addr,
                                message: tokio_tungstenite::tungstenite::Message::Binary(bin),
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    tokio_tungstenite::tungstenite::Message::Close(_) => {
                        break;
                    }
                    _ => {}
                },
                Some(Err(_e)) => {
                    break;
                }
                None => {
                    break;
                }
            }
        }

        clients.lock().await.remove(&addr);

        let _ = event_tx.send(WebsocketEvent::Disconnected { addr }).await;
    }

    pub async fn send_to(
        &self,
        addr: std::net::SocketAddr,
        msg: tokio_tungstenite::tungstenite::Message,
    ) -> bool {
        let tx = {
            let clients = self.clients.lock().await;

            clients.get(&addr).map(|client| client.tx.clone())
        };

        match tx {
            Some(tx) => {
                return tx.send(msg).await.is_ok();
            }
            None => false,
        }
    }

    pub async fn get_connected_clients(&self) -> Vec<std::net::SocketAddr> {
        let clients = self.clients.lock().await;
        clients.keys().cloned().collect()
    }

    fn select_subprotocol(
        request: &tokio_tungstenite::tungstenite::handshake::server::Request,
    ) -> Option<&'static str> {
        let header_protocol = request
            .headers()
            .get(tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL);

        if let Some(header_protocol) = header_protocol
            && let Ok(header_protocol_str) = header_protocol.to_str()
        {
            for supported in SUPPORTED_SUBPROTOCOLS {
                for requested in header_protocol_str.split(',') {
                    if requested.trim() == *supported {
                        return Some(*supported);
                    }
                }
            }
        }

        None
    }

    pub async fn publish_pointcloud(
        &self,
        channel_id: ChannelId,
        frame: &PointCloudFrame,
    ) -> Result<(), serde_json::Error> {
        let channel = match self.channels.get(channel_id) {
            Some(channel) => channel,
            None => return Ok(()),
        };

        if frame.point_step != channel.layout.point_step || frame.fields != channel.layout.fields {
            return Ok(());
        }

        let targets = {
            let clients = self.clients.lock().await;
            let mut foxgloves_targets = vec![];
            let mut wire_targets = vec![];

            for client in clients.values() {
                for (subscription_id, subscribed_channel_id) in &client.subscriptions {
                    if *subscribed_channel_id == channel_id {
                        match client.subprotocol {
                            Some(FOXGLOVE_SUBPROTOCOL) => {
                                foxgloves_targets.push((client.tx.clone(), *subscription_id));
                            }
                            Some(POINTCLOUD_WIRE_SUBPROTOCOL) => {
                                wire_targets.push((client.tx.clone(), *subscription_id));
                            }
                            _ => {}
                        }
                    }
                }
            }

            (foxgloves_targets, wire_targets)
        };

        if targets.0.is_empty() && targets.1.is_empty() {
            return Ok(());
        }

        let point_data = pack_point_data(frame);

        // foxgloves targets
        if !targets.0.is_empty()
            && let Ok(payload) = encode_foxglove_pointcloud_payload(frame, &point_data)
        {
            for (tx, subscription_id) in targets.0 {
                let message = encode_foxglove_pointcloud_message(
                    subscription_id,
                    frame.timestamp_ns,
                    &payload,
                );

                let _ = tx
                    .send(tokio_tungstenite::tungstenite::Message::Binary(
                        message.into(),
                    ))
                    .await;
            }
        }

        // wire targets
        for (tx, subscription_id) in targets.1 {
            let message =
                encode_wire_pointcloud_message(subscription_id, frame.timestamp_ns, &point_data);
            let _ = tx
                .send(tokio_tungstenite::tungstenite::Message::Binary(
                    message.into(),
                ))
                .await;
        }

        Ok(())
    }
}

impl Default for WebsocketServer {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:18899".to_string(),
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            channels: ChannelRegistry::new(),
        }
    }
}
