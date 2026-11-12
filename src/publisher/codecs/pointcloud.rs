//! Point Cloud 메시지의 Wire payload 인코딩.

use serde_json::{Map, Value, json};

use crate::pointcloud::{PointCloudFrame, PointCloudLayout, pack_point_data};
use crate::publisher::protocols::wire::WireSchema;
use crate::publisher::{MessageCodec, PublishError};

const POINTCLOUD_SCHEMA_NAME: &str = "pointcloud_stream.PointCloud";

#[derive(Clone)]
pub(crate) struct PointCloudCodec {
    layout: PointCloudLayout,
}

impl PointCloudCodec {
    pub(crate) fn new(layout: PointCloudLayout) -> Self {
        Self { layout }
    }
}

impl MessageCodec for PointCloudCodec {
    type Message = PointCloudFrame;

    fn schema(&self) -> WireSchema {
        WireSchema::new(POINTCLOUD_SCHEMA_NAME, 1, "binary")
    }

    fn subscription_metadata(&self) -> Map<String, Value> {
        let fields: Vec<Value> = self
            .layout
            .fields
            .iter()
            .map(|field| {
                json!({
                    "name": field.name,
                    "offset": field.offset,
                    "dataType": field.data_type as u8,
                    "count": field.count,
                })
            })
            .collect();

        let mut metadata = Map::new();
        metadata.insert("pointStride".into(), json!(self.layout.point_step));
        metadata.insert("fields".into(), json!(fields));

        metadata
    }

    fn timestamp_ns(&self, message: &PointCloudFrame) -> u64 {
        message.timestamp_ns
    }

    fn validate(&self, message: &PointCloudFrame) -> Result<(), PublishError> {
        if message.point_step != self.layout.point_step || message.fields != self.layout.fields {
            return Err(PublishError::LayoutMismatch);
        }

        Ok(())
    }

    fn encode(&self, message: &PointCloudFrame) -> Result<Vec<u8>, PublishError> {
        Ok(pack_point_data(message))
    }
}
