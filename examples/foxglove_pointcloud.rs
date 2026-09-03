use std::time::Duration;

use pointcloud_stream::pointcloud::{
    Endianness, PointCloudFrame, PointCloudLayout, PointField, PointFieldDataType,
};
use pointcloud_stream::publisher::websocket::{WebsocketEvent, WebsocketServer};

#[tokio::main]
async fn main() {
    let mut server = WebsocketServer::new("127.0.0.1:18282");

    let layout = PointCloudLayout {
        point_step: 12,
        fields: vec![
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
    };

    let pointcloud_channel = server.register_channel("/pointcloud", layout);

    let running_server = server.clone();

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<WebsocketEvent>(32);

    tokio::spawn(async move {
        running_server.run(ready_tx, event_tx).await.unwrap();
    });

    let address = ready_rx.await.unwrap();

    println!("WebSocket server: ws://{}", address);

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            println!("{:?}", event);
        }
    });

    let mut value = 0.0f32;

    loop {
        let frame = create_pointcloud_frame(value);

        server
            .publish_pointcloud(pointcloud_channel, &frame)
            .await
            .unwrap();

        value += 0.05;

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn create_pointcloud_frame(offset: f32) -> PointCloudFrame {
    let point_count = 100u32;

    let mut data = Vec::new();

    for i in 0..point_count {
        let x = i as f32 * 0.05;
        let y = offset.sin();
        let z = offset.cos();

        data.extend_from_slice(&x.to_le_bytes());
        data.extend_from_slice(&y.to_le_bytes());
        data.extend_from_slice(&z.to_le_bytes());
    }

    let timestamp_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    PointCloudFrame {
        timestamp_ns: timestamp_ns,
        frame_id: "map".to_string(),

        width: point_count,
        height: 1,

        fields: vec![
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

        endianness: Endianness::Little,
        point_step: 12,
        row_step: point_count * 12,
        is_dense: true,
        data,
    }
}
