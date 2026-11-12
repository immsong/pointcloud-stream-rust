# pointcloud-stream

실시간 Point Cloud와 Detection 정보를 공통 형식으로 표현하고, 가공 및 스트리밍하기 위한 Rust 라이브러리입니다.

현재 버전은 **0.2.0**입니다. 릴리스별 변경 사항은 [CHANGELOG.md](CHANGELOG.md)를 참고합니다.

센서 또는 장치별 통신과 프로토콜 파싱은 라이브러리의 범위에서 제외합니다.  
외부에서 생성한 `PointCloudFrame`과 `DetectionInfo`를 전달받아 검증, 변환, 패킹 및 WebSocket 스트리밍을 처리하는 데 초점을 둡니다.

## Features

### Point Cloud

- `PointCloudFrame`, `PointField`, `PointFieldDataType`
- Point Cloud layout 표현 및 검증
- `PointCloudBuilder`를 이용한 frame 생성
- Little / Big Endian 지원
- row padding 및 point padding을 고려한 데이터 처리

### Detection

- `DetectionInfo`, `DetectionArea`, `DetectionStatus`, `DetectionAreaKind`
- 하나의 Detection 메시지에 여러 감지 영역과 감지 상태 표현
- 평면 다각형 `Polygon2D` 및 회전된 직육면체를 포함한 `Cube3D` 지원
- 좌표 유효성, 중복 꼭짓점, 평면성, 다각형 교차 및 직육면체 형상 검증
- `DetectionArea::normalize()`를 통한 꼭짓점 순서 정규화
- Wire v2를 통한 Detection binary message 발행

Detection은 외부에서 계산한 감지 영역과 결과를 표현하는 데이터 모델입니다. 객체 검출 알고리즘은 제공하지 않습니다.

### Transform

- Translation
- Roll / Pitch / Yaw 회전
- Point 단위 좌표 변환
- `PointCloudFrame` 전체 좌표 변환

### Streaming

- `LatestFrameStream`을 통한 최신 Point Cloud frame 유지
- 느린 consumer 환경에서 중간 frame을 누적하지 않는 방식
- 공통 Point Cloud packing
  - row padding 제거
  - Little Endian 정규화
- 서버 복제본 사이에서 공유하는 channel registry
- Point Cloud와 Detection의 검증 및 인코딩을 처리하는 공통 내부 Codec 구조

## WebSocket Publisher

하나의 WebSocket 서버에서 subprotocol에 따라 서로 다른 전송 방식을 지원합니다.

| 프로토콜 | WebSocket subprotocol | 지원 데이터 |
| --- | --- | --- |
| Foxglove | `foxglove.websocket.v1` | Point Cloud |
| PointCloud Wire v2 | `pointcloud.wire.v2` | Point Cloud, Detection |

### Channel

Point Cloud와 Detection은 각각 별도의 topic과 channel ID로 등록합니다.
같은 장치 또는 데이터 소스에 속한 채널에는 동일한 `instance_id`를 지정할 수 있습니다.

| 데이터 | Topic 예시 | `instance_id` 예시 | Wire `schemaName` |
| --- | --- | --- | --- |
| Point Cloud | `/pointcloud/lidar-1` | `lidar-1` | `pointcloud_stream.PointCloud` |
| Detection | `/pointcloud/lidar-1/detection` | `lidar-1` | `pointcloud_stream.DetectionInfo` |

같은 `instance_id`를 사용하더라도 구독과 발행은 채널별로 독립적으로 처리합니다.
클라이언트 연결 시 채널 목록이 전달되므로, 사용할 채널은 연결 전에 등록합니다.

`WebsocketServer`는 다음 API를 제공합니다.

| API | 용도 |
| --- | --- |
| `register_pointcloud_channel(instance_id, topic, layout)` | Point Cloud 채널 등록 |
| `register_detection_channel(instance_id, topic)` | Detection 채널 등록 |
| `publish_pointcloud(channel_id, &frame)` | Point Cloud frame 발행 |
| `publish_detection(channel_id, &info)` | Detection 정보 발행 |

위 API는 `Result`를 반환합니다. 기존 `register_channel(instance_id, topic, layout)`도 유지하며, 이 함수는 `ChannelId`를 직접 반환하고 등록 실패 시 panic이 발생합니다.

### Foxglove

Subprotocol:

```text
foxglove.websocket.v1
```

지원 기능:

- server info
- channel advertise
- subscribe / unsubscribe
- `foxglove.PointCloud` JSON payload
- Point Cloud binary message 발행

Foxglove 클라이언트에는 Point Cloud 채널만 안내합니다.
Detection 채널은 Wire v2에서 사용합니다.

### PointCloud Wire Protocol v2

Subprotocol:

```text
pointcloud.wire.v2
```

Point Cloud와 Detection 전송을 위한 경량 WebSocket protocol입니다.
Control message는 JSON을 사용하고, 데이터는 스키마별 binary payload로 전송합니다.

각 채널은 `schemaName`, `schemaVersion`, `encoding`으로 메시지 형식을 설명합니다.
현재 제공하는 두 스키마의 `schemaVersion`은 `1`이며, `encoding`은 `binary`입니다.
이 스키마 버전은 WebSocket subprotocol의 버전인 v2와 별개입니다.

#### Channel List

연결 직후 서버는 채널 목록을 전송합니다.

```json
{
  "op": "channels",
  "channels": [
    {
      "id": 0,
      "topic": "/pointcloud/lidar-1",
      "schemaName": "pointcloud_stream.PointCloud",
      "schemaVersion": 1,
      "encoding": "binary",
      "instanceId": "lidar-1"
    },
    {
      "id": 1,
      "topic": "/pointcloud/lidar-1/detection",
      "schemaName": "pointcloud_stream.DetectionInfo",
      "schemaVersion": 1,
      "encoding": "binary",
      "instanceId": "lidar-1"
    }
  ]
}
```

#### Subscribe

Client는 사용할 channel을 구독합니다.
`id`는 클라이언트가 정하는 구독 ID이고, `channelId`는 서버가 안내한 채널 ID입니다.

```json
{
  "op": "subscribe",
  "subscriptions": [
    { "id": 10, "channelId": 0 },
    { "id": 11, "channelId": 1 }
  ]
}
```

서버는 수락한 구독의 스키마 정보를 반환합니다.
Point Cloud 채널에는 `pointStride`와 `fields`를 함께 전달합니다.

```json
{
  "op": "subscribed",
  "subscriptions": [
    {
      "id": 10,
      "channelId": 0,
      "schemaName": "pointcloud_stream.PointCloud",
      "schemaVersion": 1,
      "encoding": "binary",
      "pointStride": 12,
      "fields": [
        { "name": "x", "offset": 0, "dataType": 7, "count": 1 },
        { "name": "y", "offset": 4, "dataType": 7, "count": 1 },
        { "name": "z", "offset": 8, "dataType": 7, "count": 1 }
      ]
    },
    {
      "id": 11,
      "channelId": 1,
      "schemaName": "pointcloud_stream.DetectionInfo",
      "schemaVersion": 1,
      "encoding": "binary"
    }
  ]
}
```

#### Unsubscribe

구독 해제에는 채널 ID가 아닌 구독 ID를 사용합니다.

```json
{
  "op": "unsubscribe",
  "subscriptionIds": [10, 11]
}
```

Wire의 `subscribe` / `unsubscribe` 외 text message는 `WebsocketEvent::Message`로 전달하므로, 응용 프로그램에서 사용자 정의 메시지를 처리할 수 있습니다.

#### Binary Message

Point Cloud와 Detection은 공통 13바이트 헤더를 사용합니다.

| Offset | 필드 | 형식 | 의미 |
| --- | --- | --- | --- |
| 0 | `opcode` | `u8` | Message data: `0x01` |
| 1 | `subscription_id` | `u32 LE` | 클라이언트가 지정한 구독 ID |
| 5 | `timestamp_ns` | `u64 LE` | 데이터 생성 시각, nanosecond |
| 13 | `payload` | bytes | 채널 스키마에 따른 데이터 |

클라이언트는 `subscription_id`에 연결된 채널의 스키마를 확인해 payload를 해석합니다.
Binary 메시지마다 스키마 이름을 반복해서 전송하지 않습니다.

#### Point Cloud Payload

스키마 이름은 `pointcloud_stream.PointCloud`입니다.
Payload에는 구독 응답의 `pointStride`와 `fields`에 따라 해석하는 point data가 들어갑니다.

- row padding 제거
- point 내부 padding 유지
- Little Endian
- 채널별 Point Cloud layout 고정

`PointCloudFrame::validate()`로 frame 구조를 검증한 뒤 발행합니다.
발행 시에는 frame의 `point_step`과 `fields`가 등록된 layout과 일치하는지 검사합니다.

#### Detection Payload

스키마 이름은 `pointcloud_stream.DetectionInfo`입니다.
Payload는 아래 필드를 순서대로 포함하며, 정수와 부동소수점 값은 Little Endian입니다.

| 필드 | 형식 | 의미 |
| --- | --- | --- |
| `frame_id_length` | `u32` | `frame_id`의 UTF-8 byte 길이 |
| `frame_id` | bytes | 좌표계 식별자 |
| `area_count` | `u32` | 감지 영역 개수 |
| `areas` | 반복 데이터 | 아래 영역 구조를 `area_count`만큼 반복 |

각 영역의 구조:

| 필드 | 형식 | 의미 |
| --- | --- | --- |
| `id` | `u32` | 메시지 내 영역 고유 ID |
| `status` | `u8` | `Off = 0`, `On = 1` |
| `kind` | `u8` | `Polygon2D = 0`, `Cube3D = 1` |
| `point_count` | `u32` | 꼭짓점 개수 |
| `points` | `[f32; 3]` 반복 | XYZ 좌표(meter)를 `point_count`만큼 반복 |

`timestamp_ns`는 공통 헤더에 기록하므로 Detection payload에는 중복 저장하지 않습니다.
영역 ID 중복, 길이 범위 및 각 영역의 형상은 발행 전에 검사합니다.

`Polygon2D`는 같은 평면 위의 꼭짓점을 외곽선 순서로 전달해야 합니다.
`Cube3D`는 직육면체의 꼭짓점 8개를 사용하며 입력 순서는 제한하지 않습니다.

발행 시 좌표와 꼭짓점 순서를 자동으로 바꾸지 않습니다.
정규화된 순서가 필요한 경우 `DetectionArea::normalize()`를 먼저 호출합니다.
이 함수는 다각형의 첫 꼭짓점을 유지하면서 선택한 투영 평면에서 반시계 방향으로 정렬하고, 직육면체는 오른손 좌표계의 P0~P7 순서로 정렬합니다.

## Basic Example

다음 예제는 Point Cloud와 Detection 채널을 등록하고 서버를 실행한 뒤,
고정된 예제 데이터를 100 ms 간격으로 발행합니다. Ctrl+C로 종료합니다.

```rust
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use pointcloud_stream::detection::{
    DetectionArea, DetectionAreaKind, DetectionInfo, DetectionStatus,
};
use pointcloud_stream::pointcloud::{PointCloudBuilder, PointCloudLayout, PointFieldDataType};
use pointcloud_stream::publisher::websocket::WebsocketServer;
use tokio::sync::{mpsc, oneshot};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = WebsocketServer::new("127.0.0.1:18282");

    let mut frame = PointCloudBuilder::new()
        .frame_id("map")
        .width(1)
        .field("x", PointFieldDataType::Float32, 1)
        .field("y", PointFieldDataType::Float32, 1)
        .field("z", PointFieldDataType::Float32, 1)
        .build(
            [
                1.0f32.to_le_bytes(),
                2.0f32.to_le_bytes(),
                3.0f32.to_le_bytes(),
            ]
            .concat(),
        )?;

    let pointcloud_channel = server.register_pointcloud_channel(
        "lidar-1",
        "/pointcloud/lidar-1",
        PointCloudLayout::new(frame.point_step, frame.fields.clone()),
    )?;

    let detection_channel =
        server.register_detection_channel("lidar-1", "/pointcloud/lidar-1/detection")?;

    let mut area = DetectionArea {
        id: 0,
        status: DetectionStatus::On,
        kind: DetectionAreaKind::Polygon2D,
        points: vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 3.0, 0.0],
            [0.0, 3.0, 0.0],
        ],
    };
    area.normalize()?;

    let mut detection = DetectionInfo {
        timestamp_ns: 0,
        frame_id: "map".to_string(),
        areas: vec![area],
    };

    let (ready_tx, ready_rx) = oneshot::channel();
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let run_server = server.clone();

    tokio::spawn(async move {
        if let Err(error) = run_server.run(ready_tx, event_tx).await {
            eprintln!("WebSocket server error: {error}");
        }
    });

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            println!("{event:?}");
        }
    });

    println!("Listening on {}", ready_rx.await?);

    let mut interval = tokio::time::interval(Duration::from_millis(100));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let timestamp_ns = SystemTime::now()
                    .duration_since(UNIX_EPOCH)?
                    .as_nanos()
                    .try_into()?;

                frame.timestamp_ns = timestamp_ns;
                detection.timestamp_ns = timestamp_ns;

                server.publish_pointcloud(pointcloud_channel, &frame).await?;
                server.publish_detection(detection_channel, &detection).await?;
            }
            result = tokio::signal::ctrl_c() => {
                result?;
                break;
            }
        }
    }

    Ok(())
}
```

예제는 Tokio의 `macros`, `rt-multi-thread`, `net`, `sync`, `time`, `signal` 기능을 사용합니다.
`PointCloudBuilder::build()`는 생성한 frame을 검증합니다.
실제 사용 시에는 예제 좌표와 감지 결과를 센서 데이터로 교체하고, frame 구조가 바뀌면 다시 검증합니다.

Wire 클라이언트는 `pointcloud.wire.v2`로 연결한 뒤 필요한 채널을 각각 구독해야 데이터를 받을 수 있습니다.
구독자가 없는 경우에도 메시지를 검증하며, 발행 성공은 클라이언트의 수신 확인을 의미하지 않습니다.

## Migration from 0.1.0

| 항목 | 0.2.0 적용 사항 |
| --- | --- |
| Wire subprotocol | `pointcloud.wire.v2`로 변경 |
| 채널 등록 | `instance_id` 인자를 전달하고 Point Cloud / Detection 등록 API 사용 |
| 채널 구분 | `schemaName`, `schemaVersion`, `encoding` 확인 |
| Wire 모듈 경로 | `publisher::protocols::wire` 사용 |
| Wire 메시지 인코더 | 공통 payload를 받는 `encode_wire_message()` 사용 |
| Foxglove 모듈 경로 | `publisher::protocols::foxglove` 사용 가능, 기존 `publisher::foxgloves`도 유지 |

Wire v1 subprotocol은 v2로 교체되었습니다. 클라이언트도 함께 갱신해야 합니다.
Point Cloud payload와 공통 13바이트 binary 헤더 형식은 유지합니다.

## Scope

이 라이브러리는 센서별 packet parsing, USB / UDP 통신, 장치 제어 및 객체 검출 알고리즘을 직접 처리하지 않습니다.

센서별 구현에서는 raw 데이터를 `PointCloudFrame`으로 변환하고, 감지 결과를 `DetectionInfo`로 구성합니다.
이후 공통 데이터 처리와 WebSocket 전송은 `pointcloud-stream`에서 수행합니다.

Point Cloud transform은 `PointCloudFrame`에 적용됩니다.
Detection 좌표 변환이 필요한 경우 응용 프로그램에서 같은 좌표계에 맞춰 전달해야 합니다.

## License

MIT License
