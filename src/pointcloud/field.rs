use super::datatype::PointFieldDataType;

/// Point Cloud frame 내부의 하나의 field를 정의한다.
///
/// 각 field는 이름, byte offset, 데이터 타입, 요소 개수를 가진다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointField {
    pub name: String,
    pub offset: u32,
    pub data_type: PointFieldDataType,
    pub count: u32,
}
