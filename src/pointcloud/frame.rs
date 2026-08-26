use crate::pointcloud::{PointCloudError, PointField, PointFieldDataType};

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
        if self.width == 0 || self.height == 0 {
            return Err(PointCloudError::InvalidDimensions);
        }

        if self.row_step < self.width * self.point_step {
            return Err(PointCloudError::InvalidRowStep);
        }

        if self.data.len() != (self.row_step * self.height) as usize {
            return Err(PointCloudError::InvalidDataLength);
        }

        let mut coordinate_fields_found = [false; 3]; // x, y, z
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

            if field.name.eq_ignore_ascii_case("x")
                || field.name.eq_ignore_ascii_case("y")
                || field.name.eq_ignore_ascii_case("z")
            {
                if field.data_type != PointFieldDataType::Float32 {
                    return Err(PointCloudError::InvalidCoordinateFieldType {
                        field_name: field.name.clone(),
                    });
                }

                // x, y, z 좌표 필드는 반드시 count가 1이어야 한다.
                if field.count != 1 {
                    return Err(PointCloudError::InvalidFieldCount {
                        field_name: field.name.clone(),
                    });
                }

                let index = match field.name.as_str() {
                    "x" | "X" => 0,
                    "y" | "Y" => 1,
                    "z" | "Z" => 2,
                    _ => unreachable!(),
                };

                // x, X 등 대소문자 구분된 동일한 field 이름의 중복을 체크한다.
                if coordinate_fields_found[index] {
                    return Err(PointCloudError::DuplicateFieldName {
                        field_name: field.name.clone(),
                    });
                }

                coordinate_fields_found[index] = true;
            }
        }

        let index = coordinate_fields_found.iter().position(|f| *f == false);
        match index {
            Some(index) => {
                return Err(PointCloudError::MissingCoordinateField {
                    field_name: match index {
                        0 => "x".to_string(),
                        1 => "y".to_string(),
                        2 => "z".to_string(),
                        _ => unreachable!(),
                    },
                });
            }
            None => {}
        }

        for (index, field) in self.fields.iter().enumerate() {
            // 자기 자신과 이전 field는 제외하고, 뒤에 있는 field들과만 비교한다.
            for other in self.fields.iter().skip(index + 1) {
                if field.name == other.name {
                    return Err(PointCloudError::DuplicateFieldName {
                        field_name: field.name.clone(),
                    });
                }

                let field_end = field.offset + field.count * field.data_type.size_bytes() as u32;

                let other_end = other.offset + other.count * other.data_type.size_bytes() as u32;

                let overlaps = field.offset < other_end && other.offset < field_end;

                if overlaps {
                    return Err(PointCloudError::OverlappingFields {
                        first_field: field.name.clone(),
                        second_field: other.name.clone(),
                    });
                }
            }
        }

        Ok(())
    }
}
