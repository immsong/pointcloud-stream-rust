# pointcloud-stream

실시간 Point Cloud 데이터를 가공하고 스트리밍하기 위한 Rust 라이브러리입니다.

센서 또는 장치별 통신과 프로토콜 처리는 제외하고, 파싱된 Point Cloud 데이터를 공통 형식으로 표현하고 처리하는 기능에 초점을 둡니다.

## Features

현재 다음 기능을 목표로 개발하고 있습니다.

* Point Cloud Frame 및 Field 구조
* Field 기반 Point Cloud 데이터 구성 및 접근
* Point Cloud 좌표 변환
* Latest-frame 기반 실시간 스트리밍
* 느린 consumer 환경에서의 backpressure 처리
* Point Cloud 데이터 encoding 지원

## License

MIT License
