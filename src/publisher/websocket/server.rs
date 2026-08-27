use futures_util::{SinkExt, StreamExt};

pub struct WebsocketServer {
    address: String,
}

impl WebsocketServer {
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
        }
    }

    pub async fn run(&self, ready: tokio::sync::oneshot::Sender<()>) -> std::io::Result<()> {
        let listen = tokio::net::TcpListener::bind(&self.address).await?;

        // listener 생성 완료 통지.
        let _ = ready.send(());

        loop {
            // 새로운 클라이언트 연결 대기.
            let (stream, addr) = listen.accept().await?;

            tokio::spawn(async move {
                Self::handle_connection(stream, addr).await;
            });
        }
    }

    /// 웹소켓 클라이언트 연결을 처리한다.
    async fn handle_connection(stream: tokio::net::TcpStream, _addr: std::net::SocketAddr) {
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
                        if tx
                            .send(tokio_tungstenite::tungstenite::Message::Text(text))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    tokio_tungstenite::tungstenite::Message::Binary(bin) => {
                        if tx
                            .send(tokio_tungstenite::tungstenite::Message::Binary(bin))
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
    }
}

impl Default for WebsocketServer {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:18899".to_string(),
        }
    }
}

#[tokio::test]
async fn websocket_server_accepts_websocket_connection_and_receives_messages() {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();

    let server = WebsocketServer::default();
    let address = server.address.clone();

    let server_task = tokio::spawn(async move {
        server.run(ready_tx).await.unwrap();
    });

    ready_rx.await.unwrap();

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

    server_task.abort();
}
