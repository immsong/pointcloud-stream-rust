use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};

use crate::publisher::websocket::WebsocketEvent;

#[derive(Clone)]
pub struct WebsocketServer {
    address: String,
    clients: Arc<
        tokio::sync::Mutex<
            HashMap<
                std::net::SocketAddr,
                tokio::sync::mpsc::Sender<tokio_tungstenite::tungstenite::Message>,
            >,
        >,
    >,
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
        ready: tokio::sync::oneshot::Sender<()>,
        event_tx: tokio::sync::mpsc::Sender<WebsocketEvent>,
    ) -> std::io::Result<()> {
        let listen = tokio::net::TcpListener::bind(&self.address).await?;

        // listener 생성 완료 통지.
        let _ = ready.send(());

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
        clients: Arc<
            tokio::sync::Mutex<
                HashMap<
                    std::net::SocketAddr,
                    tokio::sync::mpsc::Sender<tokio_tungstenite::tungstenite::Message>,
                >,
            >,
        >,
        event_tx: tokio::sync::mpsc::Sender<WebsocketEvent>,
    ) {
        //  websocket handshake
        let ws_stream = match tokio_tungstenite::accept_async(stream).await {
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

        clients.lock().await.insert(addr, tx.clone());

        // client 등록 후 연결 완료 이벤트를 전송.
        let _ = event_tx.send(WebsocketEvent::Connected { addr }).await;

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

        loop {
            // client로부터 다음 WebSocket 메시지를 기다린다.
            let some_message = ws_reader.next().await;
            match some_message {
                Some(Ok(msg)) => match msg {
                    tokio_tungstenite::tungstenite::Message::Text(text) => {
                        if event_tx
                            .send(WebsocketEvent::Message {
                                addr,
                                message: tokio_tungstenite::tungstenite::Message::Text(text),
                            })
                            .await
                            .is_err()
                        {
                            break;
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
            clients.get(&addr).cloned()
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
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

        let server = WebsocketServer::default();
        let running_server = server.clone();
        address = server.address.clone();

        server_task = tokio::spawn(async move {
            running_server.run(ready_tx, event_tx).await.unwrap();
        });

        ready_rx.await.unwrap();

        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    WebsocketEvent::Connected { addr } => {
                        println!("Client connected: {}", addr);
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
