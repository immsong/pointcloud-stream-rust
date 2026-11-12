use crate::detection::DetectionError;
use crate::detection::geometry::{
    normalize_cube, normalize_polygon, validate_cube, validate_polygon,
};

/// 하나의 시점에 대한 감지 영역과 감지 결과.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectionInfo {
    /// Unix epoch 기준 데이터 생성 시각을 nanosecond 단위로 표현한다.
    pub timestamp_ns: u64,

    /// 감지 영역 좌표값의 기준이 되는 좌표계 식별자.
    ///
    /// `PointCloudFrame`의 좌표계 식별자와 동일한 의미이다.
    pub frame_id: String,

    /// 감지 영역 목록.
    ///
    /// 영역 개수 제한은 응용 프로그램에서 정의한다.
    pub areas: Vec<DetectionArea>,
}

/// 하나의 감지 영역과 감지 결과.
#[derive(Clone, Debug, PartialEq)]
pub struct DetectionArea {
    /// 영역 고유 ID.
    pub id: u32,

    /// 감지 결과.
    pub status: DetectionStatus,

    /// 감지 영역의 형상.
    pub kind: DetectionAreaKind,

    /// 감지 영역을 구성하는 XYZ 좌표(meter).
    ///
    /// - `Polygon2D`: 외곽선 순서로 나열한 3개 이상의 꼭짓점.
    /// - `Cube3D`: 직육면체를 구성하는 8개의 꼭짓점. 입력 순서는 제한하지 않는다.
    pub points: Vec<[f32; 3]>,
}

impl DetectionArea {
    /// 입력 좌표와 순서를 유지하면서 감지 영역의 형상을 검증한다.
    pub fn validate(&self) -> Result<(), DetectionError> {
        match self.kind {
            DetectionAreaKind::Polygon2D => validate_polygon(&self.points),
            DetectionAreaKind::Cube3D => validate_cube(&self.points),
        }
    }

    /// 감지 영역의 형상을 검증하고 꼭짓점 순서를 정규화한다.
    ///
    /// 좌표값은 유지하며, 검증에 실패하면 기존 `points`를 변경하지 않는다.
    ///
    /// - `Polygon2D`: 첫 꼭짓점을 고정하고, 선택한 투영 평면에서 반시계 방향(CCW)으로 정렬한다.
    /// - `Cube3D`: 오른손 좌표계를 기준으로 꼭짓점을 P0~P7 순서로 정렬한다.
    pub fn normalize(&mut self) -> Result<(), DetectionError> {
        let normalized = match self.kind {
            DetectionAreaKind::Polygon2D => normalize_polygon(&self.points)?,
            DetectionAreaKind::Cube3D => normalize_cube(&self.points)?,
        };

        self.points = normalized;

        Ok(())
    }
}

/// 감지 영역의 감지 상태.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DetectionStatus {
    Off = 0,
    On = 1,
}

/// 감지 영역에서 지원하는 형상.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DetectionAreaKind {
    /// 하나의 평면에 놓인 다각형.
    Polygon2D = 0,

    /// 회전된 형상을 포함한 직육면체.
    Cube3D = 1,
}
