//! Detection 메시지의 Wire payload 인코딩.

use std::collections::HashSet;

use crate::detection::DetectionInfo;
use crate::publisher::protocols::wire::WireSchema;
use crate::publisher::{MessageCodec, PublishError};

const DETECTION_SCHEMA_NAME: &str = "pointcloud_stream.DetectionInfo";

#[derive(Clone, Copy, Default)]
pub(crate) struct DetectionCodec;

impl MessageCodec for DetectionCodec {
    type Message = DetectionInfo;

    fn schema(&self) -> WireSchema {
        WireSchema::new(DETECTION_SCHEMA_NAME, 1, "binary")
    }

    fn timestamp_ns(&self, message: &DetectionInfo) -> u64 {
        message.timestamp_ns
    }

    fn validate(&self, message: &DetectionInfo) -> Result<(), PublishError> {
        validate_detection(message)
    }

    fn encode(&self, message: &DetectionInfo) -> Result<Vec<u8>, PublishError> {
        // 공통 발행 경로에서 검증을 마친 메시지를 인코딩한다.
        Ok(encode_validated_detection(message))
    }
}

fn validate_detection(info: &DetectionInfo) -> Result<(), PublishError> {
    validate_length(info.frame_id.len(), "frame_id")?;
    validate_length(info.areas.len(), "areas")?;

    let mut area_ids = HashSet::new();

    // 인코딩 전에 모든 영역을 검증하며, 꼭짓점 순서는 변경하지 않는다.
    for area in &info.areas {
        if !area_ids.insert(area.id) {
            return Err(PublishError::DuplicateAreaId(area.id));
        }

        validate_length(area.points.len(), "points")?;
        area.validate()
            .map_err(|source| PublishError::InvalidArea {
                area_id: area.id,
                source,
            })?;
    }

    Ok(())
}

fn validate_length(value: usize, field: &'static str) -> Result<u32, PublishError> {
    u32::try_from(value).map_err(|_| PublishError::LengthOverflow(field))
}

fn encode_validated_detection(info: &DetectionInfo) -> Vec<u8> {
    let frame_id_len = info.frame_id.len() as u32;
    let area_count = info.areas.len() as u32;

    let mut payload = Vec::new();

    payload.extend_from_slice(&frame_id_len.to_le_bytes());
    payload.extend_from_slice(info.frame_id.as_bytes());
    payload.extend_from_slice(&area_count.to_le_bytes());

    for area in &info.areas {
        payload.extend_from_slice(&area.id.to_le_bytes());
        payload.push(area.status as u8);
        payload.push(area.kind as u8);
        payload.extend_from_slice(&(area.points.len() as u32).to_le_bytes());

        for point in &area.points {
            for coordinate in point {
                payload.extend_from_slice(&coordinate.to_le_bytes());
            }
        }
    }

    payload
}
