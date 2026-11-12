//! Wire payload를 Foxglove 메시지로 변환하는 선택적 어댑터.

use std::any::Any;

use crate::pointcloud::PointCloudFrame;
use crate::publisher::PublishError;
use crate::publisher::protocols::foxglove::encode_foxglove_pointcloud_payload;

pub(crate) trait FoxgloveAdapter: Send + Sync {
    fn schema_name(&self) -> &'static str;

    fn encoding(&self) -> &'static str;

    fn schema(&self) -> &'static str;

    fn encode(&self, message: &dyn Any, wire_payload: &[u8]) -> Result<Vec<u8>, PublishError>;
}

pub(crate) struct PointCloudAdapter;

impl FoxgloveAdapter for PointCloudAdapter {
    fn schema_name(&self) -> &'static str {
        "foxglove.PointCloud"
    }

    fn encoding(&self) -> &'static str {
        "json"
    }

    fn schema(&self) -> &'static str {
        ""
    }

    fn encode(&self, message: &dyn Any, point_data: &[u8]) -> Result<Vec<u8>, PublishError> {
        let frame = message.downcast_ref::<PointCloudFrame>().ok_or_else(|| {
            PublishError::Encoding(
                "Foxglove PointCloud adapter received the wrong Rust type".into(),
            )
        })?;

        encode_foxglove_pointcloud_payload(frame, point_data)
            .map_err(|error| PublishError::Encoding(error.to_string()))
    }
}
