use crate::pointcloud::PointField;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PointCloudLayout {
    pub point_step: u32,
    pub fields: Vec<PointField>,
}

impl PointCloudLayout {
    pub fn new(point_step: u32, fields: Vec<PointField>) -> Self {
        Self { point_step, fields }
    }
}
