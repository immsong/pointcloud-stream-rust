/// Point Cloud field에서 사용하는 원시 데이터 타입.
///
/// 각 값은 ROS `sensor_msgs/PointField`의 datatype 값과 동일하게 정의한다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PointFieldDataType {
    Int8 = 1,
    Uint8 = 2,
    Int16 = 3,
    Uint16 = 4,
    Int32 = 5,
    Uint32 = 6,
    Float32 = 7,
    Float64 = 8,
}

impl PointFieldDataType {
    /// 데이터 타입 하나가 차지하는 byte 크기를 반환한다.
    pub const fn size_bytes(self) -> usize {
        match self {
            Self::Int8 | Self::Uint8 => 1,
            Self::Int16 | Self::Uint16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Float64 => 8,
        }
    }
}
