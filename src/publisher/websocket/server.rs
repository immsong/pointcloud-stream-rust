use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};

use crate::pointcloud::{PointCloudFrame, PointCloudLayout};
use crate::publisher::foxgloves::{
    Advertise, FOXGLOVE_OP_SUBSCRIBE, FOXGLOVE_OP_UNSUBSCRIBE, FOXGLOVE_SUBPROTOCOL,
    FoxgloveOperation, ServerInfo, Subscribe as FoxgloveSubscribe,
    Unsubscribe as FoxgloveUnsubscribe, encode_message_data, encode_pointcloud_payload,
};
use crate::publisher::pointcloud_wire::{
    ChannelList, POINTCLOUD_WIRE_SUBPROTOCOL, Subscribe as WireSubscribe, Subscribed,
    SubscribedChannel, Unsubscribe as WireUnsubscribe, WIRE_OP_SUBSCRIBE, WIRE_OP_UNSUBSCRIBE,
    WireOperation,
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
            None => return false,
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

        if let Some(header_protocol) = header_protocol {
            if let Ok(header_protocol_str) = header_protocol.to_str() {
                for supported in SUPPORTED_SUBPROTOCOLS {
                    for requested in header_protocol_str.split(',') {
                        if requested.trim() == *supported {
                            return Some(*supported);
                        }
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
        if self.channels.get(channel_id).is_none() {
            return Ok(());
        }

        let targets = {
            let clients = self.clients.lock().await;
            let mut foxgloves_targets = vec![];
            let wire_targets: Vec<(
                tokio::sync::mpsc::Sender<tokio_tungstenite::tungstenite::Message>,
                u32,
            )> = vec![];

            for client in clients.values() {
                match client.subprotocol {
                    Some(FOXGLOVE_SUBPROTOCOL) => {
                        // subscription_id
                        // : Foxglove client가 부여한 subscription ID.
                        //
                        // subscribed_channel_id
                        //: 해당 subscription이 구독 중인 공통 ChannelId.
                        //
                        // channel_id
                        // : publish_pointcloud()로 전달된, 지금 전송하려는 PointCloudFrame의 ChannelId.
                        //
                        // 현재 publish하려는 channel을 구독 중인 subscription만 전송 대상으로 추가한다.
                        for (subscription_id, subscribed_channel_id) in &client.subscriptions {
                            if *subscribed_channel_id == channel_id {
                                foxgloves_targets.push((client.tx.clone(), *subscription_id));
                            }
                        }
                    }
                    Some(POINTCLOUD_WIRE_SUBPROTOCOL) => {
                        // TODO: wire encoder 구현 후 추가
                    }
                    _ => {}
                }
            }

            (foxgloves_targets, wire_targets)
        };

        if targets.0.is_empty() && targets.1.is_empty() {
            return Ok(());
        }

        let payload = encode_pointcloud_payload(frame)?;

        // foxgloves targets
        for (tx, subscription_id) in targets.0 {
            let message = encode_message_data(subscription_id, frame.timestamp_ns, &payload);

            let _ = tx
                .send(tokio_tungstenite::tungstenite::Message::Binary(
                    message.into(),
                ))
                .await;
        }

        // wire targets
        for (_tx, _subscription_id) in targets.1 {
            // TODO: wire encoder 구현 후 추가
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

#[tokio::test]
async fn websocket_server_accepts_websocket_connection_and_receives_messages() {
    let address;
    let server_task;
    // server
    {
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<std::net::SocketAddr>();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

        let server = WebsocketServer::new("127.0.0.1:0");
        let running_server = server.clone();

        server_task = tokio::spawn(async move {
            running_server.run(ready_tx, event_tx).await.unwrap();
        });

        address = ready_rx.await.unwrap();

        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    WebsocketEvent::Connected { addr, subprotocol } => {
                        println!("Client connected: {}, subprotocol: {:?}", addr, subprotocol);
                    }
                    WebsocketEvent::Disconnected { addr } => {
                        println!("Client disconnected: {}", addr);
                    }
                    WebsocketEvent::Message { addr, message } => {
                        println!("Received message from {}: {}", addr, message);
                        server.send_to(addr, message).await;
                    }
                }
            }
        });
    }

    // client
    {
        let url = format!("ws://{}", address);
        let (mut websocket, _response) = tokio_tungstenite::connect_async(url).await.unwrap();

        websocket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                "Hello, server!".to_string().into(),
            ))
            .await
            .unwrap();

        let received_message = websocket.next().await.unwrap().unwrap();
        match received_message {
            tokio_tungstenite::tungstenite::Message::Text(text) => {
                assert_eq!(text, "Hello, server!");
            }
            _ => panic!("Unexpected message type"),
        }

        websocket
            .send(tokio_tungstenite::tungstenite::Message::Binary(
                tokio_tungstenite::tungstenite::Bytes::from([1, 2, 3, 9, 99].to_vec()),
            ))
            .await
            .unwrap();

        let received_message = websocket.next().await.unwrap().unwrap();
        match received_message {
            tokio_tungstenite::tungstenite::Message::Binary(bin) => {
                assert_eq!(
                    bin,
                    tokio_tungstenite::tungstenite::Bytes::from([1, 2, 3, 9, 99].to_vec())
                );
            }
            _ => panic!("Unexpected message type"),
        }
    }

    server_task.abort();
}

#[tokio::test]
async fn websocket_server_accepts_supported_subprotocol() {
    use crate::publisher::foxgloves::AdvertisedChannel;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    // server
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<std::net::SocketAddr>();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

    let mut server = WebsocketServer::new("127.0.0.1:0");

    let pointcloud_channel =
        server.register_channel("/pointcloud", PointCloudLayout::new(0, Vec::new()));

    let running_server = server.clone();

    let server_task = tokio::spawn(async move {
        running_server.run(ready_tx, event_tx).await.unwrap();
    });

    let address = ready_rx.await.unwrap();
    println!("Server is listening on {}", address);

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                WebsocketEvent::Connected { addr, subprotocol } => {
                    println!("Client connected: {}, subprotocol: {:?}", addr, subprotocol);
                }
                WebsocketEvent::Disconnected { addr } => {
                    println!("Client disconnected: {}", addr);
                }
                WebsocketEvent::Message { addr, message } => {
                    println!("Received message from {}: {}", addr, message);
                    server.send_to(addr, message).await;
                }
            }
        }
    });

    // client
    let url = format!("ws://{}", address);

    let mut request = url.into_client_request().unwrap();

    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL,
        tokio_tungstenite::tungstenite::http::HeaderValue::from_static("foxglove.websocket.v1"),
    );

    let (mut websocket, response) = tokio_tungstenite::connect_async(request).await.unwrap();

    let subprotocol = response
        .headers()
        .get(tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL)
        .unwrap();

    assert_eq!(subprotocol, "foxglove.websocket.v1");

    let received_message = websocket.next().await.unwrap().unwrap();
    match received_message {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            let received: serde_json::Value = serde_json::from_str(&text).unwrap();

            let expected = serde_json::to_value(ServerInfo::new("pointcloud-stream")).unwrap();

            assert_eq!(received, expected);
        }
        _ => panic!("Unexpected message type"),
    }

    let received_message = websocket.next().await.unwrap().unwrap();

    match received_message {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            let received: serde_json::Value = serde_json::from_str(&text).unwrap();

            let expected = serde_json::to_value(Advertise::new(vec![AdvertisedChannel {
                id: pointcloud_channel.as_u32(),
                topic: "/pointcloud".to_string(),
                encoding: "json".to_string(),
                schema_name: "foxglove.PointCloud".to_string(),
                schema: "".to_string(),
            }]))
            .unwrap();
            assert_eq!(received, expected);
        }
        _ => panic!("Unexpected message type"),
    }

    server_task.abort();
}

#[tokio::test]
async fn websocket_server_accepts_pointcloud_wire_subprotocol() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<std::net::SocketAddr>();

    let (event_tx, _event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

    let mut server = WebsocketServer::new("127.0.0.1:0");

    let front_channel =
        server.register_channel("/lidar/front", PointCloudLayout::new(0, Vec::new()));

    let rear_channel = server.register_channel("/lidar/rear", PointCloudLayout::new(0, Vec::new()));

    let running_server = server.clone();

    let server_task = tokio::spawn(async move {
        running_server.run(ready_tx, event_tx).await.unwrap();
    });

    let address = ready_rx.await.unwrap();

    let url = format!("ws://{}", address);

    let mut request = url.into_client_request().unwrap();

    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL,
        tokio_tungstenite::tungstenite::http::HeaderValue::from_static(POINTCLOUD_WIRE_SUBPROTOCOL),
    );

    let (mut websocket, response) = tokio_tungstenite::connect_async(request).await.unwrap();

    // WebSocket handshake에서 PointCloud Wire subprotocol이
    // 정상적으로 선택되었는지 확인한다.
    let subprotocol = response
        .headers()
        .get(tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL)
        .unwrap();

    assert_eq!(subprotocol, POINTCLOUD_WIRE_SUBPROTOCOL,);

    // Wire 연결 직후 서버가 전송하는 channel 목록을 확인한다.
    let received_message = websocket.next().await.unwrap().unwrap();

    match received_message {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();

            assert_eq!(json["op"], "channels");

            let channels = json["channels"].as_array().unwrap();

            assert_eq!(channels.len(), 2);

            assert_eq!(channels[0]["id"], front_channel.as_u32());

            assert_eq!(channels[0]["topic"], "/lidar/front");

            assert_eq!(channels[1]["id"], rear_channel.as_u32());

            assert_eq!(channels[1]["topic"], "/lidar/rear");
        }

        _ => {
            panic!("Expected channel list message");
        }
    }

    server_task.abort();
}

#[tokio::test]
async fn websocket_server_handles_pointcloud_wire_subscription() {
    use crate::pointcloud::{PointField, PointFieldDataType};
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<std::net::SocketAddr>();

    let (event_tx, _event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

    let mut server = WebsocketServer::new("127.0.0.1:0");

    let layout = PointCloudLayout::new(
        12,
        vec![
            PointField {
                name: "x".to_string(),
                offset: 0,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            PointField {
                name: "y".to_string(),
                offset: 4,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            PointField {
                name: "z".to_string(),
                offset: 8,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
        ],
    );

    let pointcloud_channel = server.register_channel("/pointcloud", layout);

    let running_server = server.clone();

    let server_task = tokio::spawn(async move {
        running_server.run(ready_tx, event_tx).await.unwrap();
    });

    let address = ready_rx.await.unwrap();

    let url = format!("ws://{}", address);
    let mut request = url.into_client_request().unwrap();

    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL,
        tokio_tungstenite::tungstenite::http::HeaderValue::from_static(POINTCLOUD_WIRE_SUBPROTOCOL),
    );

    let (mut websocket, _) = tokio_tungstenite::connect_async(request).await.unwrap();

    // 연결 직후 ChannelList 소비.
    let channel_list = websocket.next().await.unwrap().unwrap();

    assert!(matches!(
        channel_list,
        tokio_tungstenite::tungstenite::Message::Text(_)
    ));

    let subscription_id = 10u32;

    let subscribe = serde_json::json!({
        "op": "subscribe",
        "subscriptions": [
            {
                "id": subscription_id,
                "channelId": pointcloud_channel.as_u32()
            }
        ]
    });

    websocket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            subscribe.to_string().into(),
        ))
        .await
        .unwrap();

    // subscribe 처리 후 서버가 전송하는 subscribed 응답을 확인한다.
    let received_message =
        tokio::time::timeout(std::time::Duration::from_secs(1), websocket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();

    match received_message {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();

            assert_eq!(json["op"], "subscribed");

            let subscriptions = json["subscriptions"].as_array().unwrap();

            assert_eq!(subscriptions.len(), 1);

            let subscription = &subscriptions[0];

            assert_eq!(subscription["id"].as_u64().unwrap(), subscription_id as u64);

            assert_eq!(
                subscription["channelId"].as_u64().unwrap(),
                pointcloud_channel.as_u32() as u64
            );

            assert_eq!(subscription["pointStride"].as_u64().unwrap(), 12);

            let fields = subscription["fields"].as_array().unwrap();

            assert_eq!(fields.len(), 3);

            assert_eq!(fields[0]["name"], "x");
            assert_eq!(fields[0]["offset"], 0);
            assert_eq!(fields[0]["dataType"], 7);
            assert_eq!(fields[0]["count"], 1);

            assert_eq!(fields[1]["name"], "y");
            assert_eq!(fields[1]["offset"], 4);
            assert_eq!(fields[1]["dataType"], 7);
            assert_eq!(fields[1]["count"], 1);

            assert_eq!(fields[2]["name"], "z");
            assert_eq!(fields[2]["offset"], 8);
            assert_eq!(fields[2]["dataType"], 7);
            assert_eq!(fields[2]["count"], 1);
        }

        _ => {
            panic!("Expected subscribed message");
        }
    }

    // 서버 내부 subscription 상태도 확인한다.
    let subscribed = {
        let clients = server.clients.lock().await;

        clients
            .values()
            .any(|client| client.subscriptions.get(&subscription_id) == Some(&pointcloud_channel))
    };

    assert!(subscribed);

    server_task.abort();
}
