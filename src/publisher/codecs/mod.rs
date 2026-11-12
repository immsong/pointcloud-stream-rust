//! Point Cloud와 Detection 메시지에 사용하는 내부 코덱.

mod detection;
mod pointcloud;

pub(crate) use detection::DetectionCodec;
pub(crate) use pointcloud::PointCloudCodec;
