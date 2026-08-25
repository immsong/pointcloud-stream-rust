use std::fmt;

/// Point Cloud frame 구조가 올바르지 않을 때 발생하는 오류.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointCloudError {
    InvalidRowStep,
    InvalidDataLength,
    InvalidFieldLayout {
        field_name: String,
    },
    InvalidFieldCount {
        field_name: String,
    },
    DuplicateFieldName {
        field_name: String,
    },
    OverlappingFields {
        first_field: String,
        second_field: String,
    },
}

impl fmt::Display for PointCloudError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRowStep => {
                write!(f, "row_step is smaller than width * point_step")?;
            }
            Self::InvalidDataLength => {
                write!(f, "data length is smaller than row_step * height")?;
            }
            Self::InvalidFieldLayout { field_name } => {
                write!(f, "field '{field_name}' exceeds point_step")?;
            }
            Self::InvalidFieldCount { field_name } => {
                write!(f, "field '{field_name}' has an invalid count")?;
            }
            Self::DuplicateFieldName { field_name } => {
                write!(f, "field '{field_name}' is duplicated")?;
            }

            Self::OverlappingFields {
                first_field,
                second_field,
            } => {
                write!(f, "fields '{first_field}' and '{second_field}' overlap")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for PointCloudError {}
