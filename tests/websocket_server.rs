use futures_util::{SinkExt, StreamExt};

use pointcloud_stream::pointcloud::PointCloudLayout;
use pointcloud_stream::publisher::foxgloves::{
    Advertise, AdvertisedChannel, FOXGLOVE_SUBPROTOCOL, ServerInfo,
};
use pointcloud_stream::publisher::websocket::{WebsocketEvent, WebsocketServer};

#[tokio::test]
async fn websocket_server_accepts_websocket_connection_and_receives_messages() {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<std::net::SocketAddr>();

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

    let server = WebsocketServer::new("127.0.0.1:0");
    let running_server = server.clone();

    let server_task = tokio::spawn(async move {
        running_server.run(ready_tx, event_tx).await.unwrap();
    });

    let address = ready_rx.await.unwrap();

    let echo_server = server.clone();

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                WebsocketEvent::Connected { .. } => {}

                WebsocketEvent::Disconnected { .. } => {}

                WebsocketEvent::Message { addr, message } => {
                    echo_server.send_to(addr, message).await;
                }
            }
        }
    });

    let url = format!("ws://{}", address);

    let (mut websocket, _) = tokio_tungstenite::connect_async(url).await.unwrap();

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

    let expected_binary = tokio_tungstenite::tungstenite::Bytes::from(vec![1, 2, 3, 9, 99]);

    websocket
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            expected_binary.clone(),
        ))
        .await
        .unwrap();

    let received_message = websocket.next().await.unwrap().unwrap();

    match received_message {
        tokio_tungstenite::tungstenite::Message::Binary(binary) => {
            assert_eq!(binary, expected_binary);
        }

        _ => panic!("Unexpected message type"),
    }

    server_task.abort();
}

#[tokio::test]
async fn websocket_server_accepts_supported_subprotocol() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<std::net::SocketAddr>();

    let (event_tx, _event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

    let mut server = WebsocketServer::new("127.0.0.1:0");

    let pointcloud_channel =
        server.register_channel("/pointcloud", PointCloudLayout::new(0, Vec::new()));

    let running_server = server.clone();

    let server_task = tokio::spawn(async move {
        running_server.run(ready_tx, event_tx).await.unwrap();
    });

    let address = ready_rx.await.unwrap();

    let url = format!("ws://{}", address);

    let mut request = url.into_client_request().unwrap();

    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL,
        tokio_tungstenite::tungstenite::http::HeaderValue::from_static(FOXGLOVE_SUBPROTOCOL),
    );

    let (mut websocket, response) = tokio_tungstenite::connect_async(request).await.unwrap();

    let subprotocol = response
        .headers()
        .get(tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL)
        .unwrap();

    assert_eq!(subprotocol, FOXGLOVE_SUBPROTOCOL);

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
