use crate::pointcloud::PointCloudError;

use super::field::PointField;

/// Point Cloud binary 데이터의 byte order를 나타낸다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endianness {
    Little,
    Big,
}

/// 하나의 Point Cloud 데이터를 표현하는 frame.
///
/// 각 point는 `fields`에 정의된 binary layout에 따라 `data`에 저장된다.
#[derive(Clone, Debug, PartialEq)]
pub struct PointCloudFrame {
    /// Frame이 생성된 시각을 nanosecond 단위로 표현한다.
    pub timestamp_ns: u64,

    /// Point Cloud 좌표값의 기준이 되는 좌표계 식별자.
    ///
    /// 예: `lidar`, `base_link`, `map`
    pub frame_id: String,

    /// Point Cloud의 가로 방향 point 개수.
    ///
    /// 센서가 2차원 격자 형태로 데이터를 제공하는 경우 가로 해상도에 해당한다.
    pub width: u32,

    /// Point Cloud의 세로 방향 point 개수.
    ///
    /// 센서가 2차원 격자 형태로 데이터를 제공하는 경우 세로 해상도에 해당한다.
    /// 1차원 point 목록 형태인 경우 일반적으로 `1`을 사용한다.
    pub height: u32,

    /// 하나의 point를 구성하는 field 목록.
    pub fields: Vec<PointField>,

    /// Point Cloud binary 데이터의 엔디안.
    pub endianness: Endianness,

    /// Point 하나의 전체 binary 데이터 크기(byte).
    ///
    /// 모든 field와 필요한 padding을 포함한 point 단위의 크기이다.
    pub point_step: u32,

    /// Point Cloud 한 행의 전체 binary 데이터 크기(byte).
    ///
    /// 일반적으로 `width * point_step`과 같으며,
    /// 행 단위 padding이 있는 경우 더 클 수 있다.
    pub row_step: u32,

    /// 모든 point의 값이 유효한지를 나타낸다.
    ///
    /// `false`인 경우 측정 실패나 유효하지 않은 센서 데이터로 인해
    /// 일부 point의 좌표 값에 `NaN` 등이 포함될 수 있다.
    pub is_dense: bool,

    /// Field layout에 따라 저장된 Point Cloud binary 데이터.
    pub data: Vec<u8>,
}

impl PointCloudFrame {
    pub fn validate(&self) -> Result<(), PointCloudError> {
        if self.row_step < self.width * self.point_step {
            return Err(PointCloudError::InvalidRowStep);
        }

        if self.data.len() != (self.row_step * self.height) as usize {
            return Err(PointCloudError::InvalidDataLength);
        }

        for field in &self.fields {
            if field.count == 0 {
                return Err(PointCloudError::InvalidFieldCount {
                    field_name: field.name.clone(),
                });
            }

            if field.offset + field.count * field.data_type.size_bytes() as u32 > self.point_step {
                return Err(PointCloudError::InvalidFieldLayout {
                    field_name: field.name.clone(),
                });
            }
        }

        Ok(())
    }
}
