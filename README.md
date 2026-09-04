# pointcloud-stream

실시간 Point Cloud 데이터를 공통 형식으로 표현하고, 가공 및 스트리밍하기 위한 Rust 라이브러리입니다.

센서 또는 장치별 통신과 프로토콜 파싱은 라이브러리의 범위에서 제외합니다.  
외부에서 파싱된 Point Cloud 데이터를 `PointCloudFrame`으로 전달받아 검증, 변환, 패킹 및 WebSocket 스트리밍을 처리하는 데 초점을 둡니다.

## Features

### Point Cloud

- `PointCloudFrame`, `PointField`, `PointFieldDataType`
- Point Cloud layout 표현 및 검증
- Builder를 이용한 Point Cloud Frame 생성
- Little / Big Endian 지원
- row padding 및 point padding을 고려한 데이터 처리

### Transform

- Translation
- Roll / Pitch / Yaw 회전
- Point 단위 좌표 변환
- PointCloudFrame 전체 좌표 변환

### Streaming

- Latest-frame 기반 스트리밍
- 느린 consumer 환경에서 중간 frame을 누적하지 않고 최신 frame 유지
- 공통 Point Cloud packing
  - row padding 제거
  - Little Endian 정규화

### WebSocket Publisher

하나의 WebSocket 서버에서 subprotocol에 따라 서로 다른 Point Cloud 전송 방식을 지원합니다.

#### Foxglove

Subprotocol:

```text
foxglove.websocket.v1
```

지원 기능:

- channel advertise
- subscribe / unsubscribe
- `foxglove.PointCloud` JSON payload
- Point Cloud binary message publish

#### PointCloud Wire Protocol

Subprotocol:

```text
pointcloud.wire.v1
```

Point Cloud 전송을 위해 정의한 경량 WebSocket protocol입니다.

Control message는 JSON을 사용하고, Point Cloud frame은 Binary로 전송합니다.

연결 직후 서버는 channel 목록을 전송합니다.

```json
{
  "op": "channels",
  "channels": [
    {
      "id": 0,
      "topic": "/pointcloud"
    }
  ]
}
```

Client는 사용할 channel을 subscribe 합니다.

```json
{
  "op": "subscribe",
  "subscriptions": [
    {
      "id": 10,
      "channelId": 0
    }
  ]
}
```

Subscribe가 완료되면 서버는 해당 channel의 Point Cloud layout을 전달합니다.

```json
{
  "op": "subscribed",
  "subscriptions": [
    {
      "id": 10,
      "channelId": 0,
      "pointStride": 12,
      "fields": [
        {
          "name": "x",
          "offset": 0,
          "dataType": 7,
          "count": 1
        },
        {
          "name": "y",
          "offset": 4,
          "dataType": 7,
          "count": 1
        },
        {
          "name": "z",
          "offset": 8,
          "dataType": 7,
          "count": 1
        }
      ]
    }
  ]
}
```

Point Cloud Binary message는 다음 구조를 사용합니다.

```text
[opcode]            u8
[subscription_id]   u32 LE
[timestamp_ns]      u64 LE
[point_data]        bytes
```

현재 Point Cloud Binary opcode:

```text
0x01
```

Binary payload의 Point Cloud 데이터는 다음 규칙으로 정규화됩니다.

- row padding 제거
- point 내부 padding 유지
- Little Endian
- channel별 Point Cloud layout 고정

## Basic Example

```rust
use pointcloud_stream::pointcloud::{
    Endianness,
    PointCloudFrame,
    PointCloudLayout,
    PointField,
    PointFieldDataType,
};
use pointcloud_stream::publisher::websocket::WebsocketServer;

#[tokio::main]
async fn main() {
    let mut server = WebsocketServer::new("127.0.0.1:18282");

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

    let channel_id = server.register_channel(
        "/pointcloud",
        PointCloudLayout::new(12, fields.clone()),
    );

    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "map".to_string(),
        width: 1,
        height: 1,
        fields,
        endianness: Endianness::Little,
        point_step: 12,
        row_step: 12,
        is_dense: true,
        data: [
            1.0f32.to_le_bytes(),
            2.0f32.to_le_bytes(),
            3.0f32.to_le_bytes(),
        ]
        .concat(),
    };

    server
        .publish_pointcloud(channel_id, &frame)
        .await
        .unwrap();
}
```

> 실제 사용 시에는 `WebsocketServer::run()`을 별도 async task에서 실행한 뒤 Point Cloud frame을 publish 합니다.

## Scope

이 라이브러리는 센서별 packet parsing, USB / UDP 통신, 장치 제어 등을 직접 처리하지 않습니다.

```text
Sensor / Driver
    ↓
PointCloudFrame
    ↓
pointcloud-stream
    ├─ validation
    ├─ transform
    ├─ packing
    └─ websocket publish
```

센서별 구현에서는 raw 데이터를 `PointCloudFrame`으로 변환하는 부분까지만 담당하고, 이후 공통 처리 및 전송은 `pointcloud-stream`에서 수행하는 구조를 권장합니다.

## License

MIT License
