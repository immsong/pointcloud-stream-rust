# Changelog

이 프로젝트의 주요 변경 사항을 기록합니다.

## [0.2.0] - 2026-11-12

Detection 지원을 추가하고 PointCloud Wire Protocol을 v2로 전환했습니다.

### Added

- `DetectionInfo`, `DetectionArea`, `DetectionStatus`, `DetectionAreaKind` 데이터 모델
- Detection 영역의 형상 검증 및 꼭짓점 순서 정규화
  - 평면 다각형 `Polygon2D` 검증 및 반시계 방향 정규화
  - 회전된 직육면체를 포함한 `Cube3D` 검증 및 꼭짓점 순서 정규화
  - 유효하지 않은 좌표, 중복 꼭짓점, 다각형 교차 등 형상 오류 검사
- Detection 채널 등록 및 `publish_detection()` API
- 채널의 `instance_id` 및 Wire 채널 목록의 `instanceId`
- Wire의 `subscribe` / `unsubscribe` 외 text message를 `WebsocketEvent::Message`로 전달

### Changed

- `WebsocketServer`와 `ChannelRegistry` 복제본이 등록된 채널을 공유하도록 개선
- Point Cloud와 Detection의 검증 및 인코딩을 공통 내부 Codec 구조로 통합
- 채널을 `schemaName`, `schemaVersion`, `encoding` 기반으로 관리
- `Result`를 반환하는 Point Cloud 등록 API `register_pointcloud()` 및 `register_pointcloud_channel()` 추가
- 공통 발행 경로에서 메시지 타입, Point Cloud layout 및 Detection 영역 검증 오류 반환
- Foxglove와 Wire의 초기 메시지 및 구독 처리를 프로토콜별 handler로 분리
- 프로토콜 분기와 전송 큐 처리를 공통화하고 관련 모듈을 `publisher::protocols` 아래로 정리
- README의 프로토콜 설명과 예제를 현재 등록 및 발행 API에 맞춰 갱신

### Breaking Changes

- Wire WebSocket subprotocol을 `pointcloud.wire.v1`에서 `pointcloud.wire.v2`로 변경
- Wire 채널 목록과 구독 응답에 스키마 정보를 포함하도록 변경
  - 클라이언트는 구독한 채널의 스키마에 따라 binary payload를 해석해야 함
- v0.1.0의 채널 등록 호출에 `instance_id` 인자 추가 필요
- Wire 공개 모듈 경로를 `publisher::pointcloud_wire`에서 `publisher::protocols::wire`로 변경
- Wire 메시지 인코더를 공통 payload용 `encode_wire_message()`로 변경

Point Cloud의 binary 데이터 형식과 13바이트 Wire 메시지 헤더는 유지합니다.
Foxglove `foxglove.websocket.v1` 지원과 기존 `publisher::foxgloves` 모듈 경로도 유지합니다.

## [0.1.0] - 2026-09-04

첫 번째 릴리스입니다.

### Added

- `PointCloudFrame`, `PointField`, `PointFieldDataType` 기본 타입
- Point Cloud frame validation
- `PointCloudBuilder`
- `PointCloudLayout`
- Point Cloud transform
  - Translation
  - Roll / Pitch / Yaw
  - Point 및 frame 단위 변환
- `LatestFrameStream`
- 공통 Point Cloud packing
  - row padding 제거
  - Big Endian → Little Endian 정규화
  - multi-value field 처리
- async WebSocket server
- WebSocket client connect / disconnect / message event 처리
- 공통 channel registry
- Foxglove `foxglove.websocket.v1` 지원
  - server info
  - channel advertise
  - subscribe / unsubscribe
  - `foxglove.PointCloud` publish
- PointCloud Wire Protocol `pointcloud.wire.v1` 지원
  - channel list
  - subscribe / unsubscribe
  - Point Cloud layout 전달
  - binary Point Cloud message publish
- WebSocket 기반 PointCloud Wire end-to-end 테스트
