use futures_util::{SinkExt, StreamExt};

use pointcloud_stream::pointcloud::{
    Endianness, PointCloudFrame, PointCloudLayout, PointField, PointFieldDataType,
};
use pointcloud_stream::publisher::pointcloud_wire::POINTCLOUD_WIRE_SUBPROTOCOL;
use pointcloud_stream::publisher::websocket::{WebsocketEvent, WebsocketServer};

use tokio_tungstenite::tungstenite::client::IntoClientRequest;

#[tokio::test]
async fn websocket_server_accepts_pointcloud_wire_subprotocol() {
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

    assert_eq!(subprotocol, POINTCLOUD_WIRE_SUBPROTOCOL);

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

    // 연결 직후 ChannelList를 소비한다.
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

    server_task.abort();
}

#[tokio::test]
async fn websocket_server_publishes_pointcloud_wire_binary_message() {
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<std::net::SocketAddr>();

    let (event_tx, _event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

    let mut server = WebsocketServer::new("127.0.0.1:0");

    let fields = vec![
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
    ];

    let layout = PointCloudLayout::new(12, fields.clone());

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

    // 연결 직후 ChannelList를 소비한다.
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

    // subscribed 응답을 받아
    // subscription 처리가 완료됐는지 확인한다.
    let subscribed_message =
        tokio::time::timeout(std::time::Duration::from_secs(1), websocket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();

    match subscribed_message {
        tokio_tungstenite::tungstenite::Message::Text(text) => {
            let json: serde_json::Value = serde_json::from_str(&text).unwrap();

            assert_eq!(json["op"], "subscribed");
        }

        _ => {
            panic!("Expected subscribed message");
        }
    }

    let x = 1.0f32;
    let y = 2.0f32;
    let z = 3.0f32;

    let mut point_data = Vec::new();

    point_data.extend_from_slice(&x.to_le_bytes());
    point_data.extend_from_slice(&y.to_le_bytes());
    point_data.extend_from_slice(&z.to_le_bytes());

    let timestamp_ns = 123456789u64;

    let frame = PointCloudFrame {
        timestamp_ns,
        frame_id: "map".to_string(),
        width: 1,
        height: 1,
        fields,
        endianness: Endianness::Little,
        point_step: 12,
        row_step: 12,
        is_dense: true,
        data: point_data.clone(),
    };

    server
        .publish_pointcloud(pointcloud_channel, &frame)
        .await
        .unwrap();

    // 실제 WebSocket을 통해 Wire Binary message를 수신한다.
    let received_message =
        tokio::time::timeout(std::time::Duration::from_secs(1), websocket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();

    match received_message {
        tokio_tungstenite::tungstenite::Message::Binary(bytes) => {
            // Wire Binary header:
            // [opcode: 1]
            // [subscription_id: 4]
            // [timestamp_ns: 8]
            assert_eq!(bytes.len(), 13 + point_data.len());

            assert_eq!(bytes[0], 0x01);

            let actual_subscription_id = u32::from_le_bytes(bytes[1..5].try_into().unwrap());

            assert_eq!(actual_subscription_id, subscription_id);

            let actual_timestamp_ns = u64::from_le_bytes(bytes[5..13].try_into().unwrap());

            assert_eq!(actual_timestamp_ns, timestamp_ns);

            assert_eq!(&bytes[13..], point_data.as_slice());

            let payload = &bytes[13..];

            let actual_x = f32::from_le_bytes(payload[0..4].try_into().unwrap());

            let actual_y = f32::from_le_bytes(payload[4..8].try_into().unwrap());

            let actual_z = f32::from_le_bytes(payload[8..12].try_into().unwrap());

            assert_eq!(actual_x, 1.0);
            assert_eq!(actual_y, 2.0);
            assert_eq!(actual_z, 3.0);
        }

        _ => {
            panic!("Expected PointCloud Wire binary message");
        }
    }

    server_task.abort();
}
