# Changelog

이 프로젝트의 주요 변경 사항을 기록합니다.

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
