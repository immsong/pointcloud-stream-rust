use std::fmt;

use crate::detection::DetectionError;
use crate::publisher::ChannelId;

/// 채널 등록 또는 메시지 발행에 실패했을 때 발생하는 오류.
#[derive(Debug)]
pub enum PublishError {
    UnknownChannel(ChannelId),
    MessageTypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    LayoutMismatch,
    InvalidSchema,
    ReservedMetadata(&'static str),
    Encoding(String),
    WrongChannelKind(ChannelId),
    InvalidArea {
        area_id: u32,
        source: DetectionError,
    },
    DuplicateAreaId(u32),
    LengthOverflow(&'static str),
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChannel(id) => {
                write!(f, "unknown channel {}", id.as_u32())?;
            }
            Self::MessageTypeMismatch { expected, actual } => {
                write!(
                    f,
                    "message type mismatch: expected {expected}, got {actual}"
                )?;
            }
            Self::LayoutMismatch => {
                write!(f, "PointCloud layout does not match the registered channel")?;
            }
            Self::InvalidSchema => {
                write!(
                    f,
                    "schema name/encoding must be nonempty and version must be positive"
                )?;
            }
            Self::ReservedMetadata(key) => {
                write!(f, "subscription metadata uses reserved field {key}")?;
            }
            Self::Encoding(error) => {
                write!(f, "message encoding failed: {error}")?;
            }
            Self::WrongChannelKind(id) => {
                write!(f, "channel {} is not correct kind of channel", id.as_u32())?;
            }
            Self::InvalidArea { area_id, source } => {
                write!(f, "invalid area {area_id}: {source}")?;
            }
            Self::DuplicateAreaId(id) => {
                write!(f, "duplicate area id {id}")?;
            }
            Self::LengthOverflow(field) => {
                write!(f, "{field} exceeds wire u32 length")?;
            }
        }

        Ok(())
    }
}

impl std::error::Error for PublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidArea { source, .. } => Some(source),
            _ => None,
        }
    }
}
