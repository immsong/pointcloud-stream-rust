use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};

use crate::publisher::foxgloves::{
    Advertise, Channel, FOXGLOVE_OP_SUBSCRIBE, FOXGLOVE_OP_UNSUBSCRIBE, FOXGLOVE_SUBPROTOCOL,
    FoxgloveOperation, ServerInfo, Subscribe, Unsubscribe,
};
use crate::publisher::websocket::WebsocketEvent;

const SUPPORTED_SUBPROTOCOLS: &[&str] = &[FOXGLOVE_SUBPROTOCOL];

#[derive(Debug)]
struct ClientState {
    tx: tokio::sync::mpsc::Sender<tokio_tungstenite::tungstenite::Message>,
    subprotocol: Option<&'static str>,
    subscriptions: HashMap<u32, u32>,
}

#[derive(Clone)]
pub struct WebsocketServer {
    address: String,
    clients: Arc<tokio::sync::Mutex<HashMap<std::net::SocketAddr, ClientState>>>,
}

impl WebsocketServer {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
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
            tokio::spawn(async move {
                Self::handle_connection(stream, addr, clients, event_tx).await;
            });
        }
    }

    /// 웹소켓 클라이언트 연결을 처리한다.
    async fn handle_connection(
        stream: tokio::net::TcpStream,
        addr: std::net::SocketAddr,
        clients: Arc<tokio::sync::Mutex<HashMap<std::net::SocketAddr, ClientState>>>,
        event_tx: tokio::sync::mpsc::Sender<WebsocketEvent>,
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

        if client_subprotocol == Some(FOXGLOVE_SUBPROTOCOL) {
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

            let advertise = Advertise::new(vec![Channel {
                id: 1,
                topic: "/pointcloud".to_string(),
                encoding: "json".to_string(),
                schema_name: "foxglove.PointCloud".to_string(),
                schema: "".to_string(),
            }]);

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
                    tokio_tungstenite::tungstenite::Message::Text(text) => {
                        if client_subprotocol == Some(FOXGLOVE_SUBPROTOCOL) {
                            if let Ok(operation) = serde_json::from_str::<FoxgloveOperation>(&text)
                            {
                                match operation.op.as_str() {
                                    FOXGLOVE_OP_SUBSCRIBE => {
                                        if let Ok(subscribe) =
                                            serde_json::from_str::<Subscribe>(&text)
                                        {
                                            let mut clients = clients.lock().await;
                                            let client_state = clients.get_mut(&addr);
                                            if let Some(client_state) = client_state {
                                                for subscription in subscribe.subscriptions {
                                                    client_state.subscriptions.insert(
                                                        subscription.id,
                                                        subscription.channel_id,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    FOXGLOVE_OP_UNSUBSCRIBE => {
                                        if let Ok(unsubscribe) =
                                            serde_json::from_str::<Unsubscribe>(&text)
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
                        } else {
                            let _ = event_tx
                                .send(WebsocketEvent::Message {
                                    addr,
                                    message: tokio_tungstenite::tungstenite::Message::Text(text),
                                })
                                .await;
                        }
                    }
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
}

impl Default for WebsocketServer {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:18899".to_string(),
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    // server
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<std::net::SocketAddr>();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

    let server = WebsocketServer::new("127.0.0.1:0");
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

            let expected = serde_json::to_value(Advertise::new(vec![Channel {
                id: 1,
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
