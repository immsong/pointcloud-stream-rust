use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};

use crate::publisher::protocols::foxglove::{self, FOXGLOVE_SUBPROTOCOL};
use crate::publisher::protocols::wire::{self, WIRE_SUBPROTOCOL};
use crate::publisher::protocols::{self as protocol, PublishTarget, Subscriptions, TextAction};
use crate::publisher::websocket::WebsocketEvent;
use crate::publisher::{ChannelId, ChannelRegistry, MessageCodec, PublishError};

const SUPPORTED_SUBPROTOCOLS: &[&str] = &[FOXGLOVE_SUBPROTOCOL, WIRE_SUBPROTOCOL];

#[derive(Debug)]
struct ClientState {
    tx: tokio::sync::mpsc::Sender<tokio_tungstenite::tungstenite::Message>,
    subprotocol: Option<&'static str>,
    subscriptions: Subscriptions,
}

#[derive(Clone)]
pub struct WebsocketServer {
    address: String,
    clients: Arc<tokio::sync::Mutex<HashMap<std::net::SocketAddr, ClientState>>>,
    pub(crate) channels: ChannelRegistry,
}

impl WebsocketServer {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            clients: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            channels: ChannelRegistry::new(),
        }
    }

    /// 내부 코덱을 사용하는 채널을 등록한다.
    ///
    /// 클라이언트에 채널 목록이 전달되도록 연결 전에 호출한다.
    pub(crate) fn register_message_channel<C: MessageCodec>(
        &self,
        instance_id: impl Into<String>,
        topic: impl Into<String>,
        codec: C,
    ) -> Result<ChannelId, PublishError> {
        self.channels.register_message(instance_id, topic, codec)
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
        // WebSocket handshake에서 사용할 하위 프로토콜을 선택한다.
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
                } else if request.headers().contains_key(
                    tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL,
                ) {
                    let mut error = tokio_tungstenite::tungstenite::http::Response::new(
                        Some("Unsupported WebSocket subprotocol".to_string()),
                    );
                    *error.status_mut() = tokio_tungstenite::tungstenite::http::StatusCode::BAD_REQUEST;
                    return Err(error);
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

        let initial_messages =
            match protocol::initial_messages(client_subprotocol, &channels.channels()) {
                Ok(messages) => messages,
                Err(_) => {
                    clients.lock().await.remove(&addr);
                    return;
                }
            };
        for message in initial_messages {
            if tx.send(message).await.is_err() {
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
                        let action = {
                            let mut clients = clients.lock().await;
                            let Some(client) = clients.get_mut(&addr) else {
                                break;
                            };
                            protocol::handle_text(
                                client_subprotocol,
                                &text,
                                &channels,
                                &mut client.subscriptions,
                            )
                        };
                        match action {
                            Ok(TextAction::Handled(replies)) => {
                                let mut failed = false;
                                for reply in replies {
                                    if tx.send(reply).await.is_err() {
                                        failed = true;
                                        break;
                                    }
                                }
                                if failed {
                                    break;
                                }
                            }
                            Ok(TextAction::Forward) => {
                                let _ = event_tx
                                    .send(WebsocketEvent::Message {
                                        addr,
                                        message: tokio_tungstenite::tungstenite::Message::Text(
                                            text,
                                        ),
                                    })
                                    .await;
                            }
                            Err(_) => continue,
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

    async fn publish_targets(
        &self,
        channel_id: ChannelId,
    ) -> (Vec<PublishTarget>, Vec<PublishTarget>) {
        let clients = self.clients.lock().await;
        let mut foxglove_targets = vec![];
        let mut wire_targets = vec![];

        for client in clients.values() {
            for (subscription_id, subscribed_channel_id) in &client.subscriptions {
                if *subscribed_channel_id == channel_id {
                    match client.subprotocol {
                        Some(FOXGLOVE_SUBPROTOCOL) => {
                            foxglove_targets.push((client.tx.clone(), *subscription_id));
                        }
                        Some(WIRE_SUBPROTOCOL) => {
                            wire_targets.push((client.tx.clone(), *subscription_id));
                        }
                        _ => {}
                    }
                }
            }
        }

        (foxglove_targets, wire_targets)
    }

    /// 등록된 코덱으로 메시지를 검증하고 구독자의 전송 큐로 전달한다.
    ///
    /// 구독자가 없어도 메시지 타입과 유효성을 검사한다.
    /// 반환값은 수신 확인을 의미하지 않으며, 전송 큐가 닫힌 경우의 오류는 반환하지 않는다.
    pub(crate) async fn publish_message<M: Send + Sync + 'static>(
        &self,
        channel_id: ChannelId,
        message: &M,
    ) -> Result<(), PublishError> {
        let channel = self
            .channels
            .get(channel_id)
            .ok_or(PublishError::UnknownChannel(channel_id))?;

        let actual = std::any::type_name::<M>();

        channel.codec.validate(message, actual)?;
        let timestamp_ns = channel.codec.timestamp_ns(message, actual)?;

        let (foxglove_targets, wire_targets) = self.publish_targets(channel_id).await;

        if foxglove_targets.is_empty() && wire_targets.is_empty() {
            return Ok(());
        }

        let payload = channel.codec.encode(message, actual)?;

        let foxglove_payload = if !foxglove_targets.is_empty() {
            match &channel.foxglove {
                Some(adapter) => Some(adapter.encode(message, &payload)?),
                None => None,
            }
        } else {
            None
        };

        if let Some(encoded) = foxglove_payload {
            foxglove::handler::publish_payload(foxglove_targets, timestamp_ns, &encoded).await;
        }

        wire::handler::publish_payload(wire_targets, timestamp_ns, &payload).await;

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
