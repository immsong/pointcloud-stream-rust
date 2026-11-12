use std::fmt;

use crate::detection::DetectionAreaKind;

/// 감지 영역의 형상이 올바르지 않을 때 발생하는 오류.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DetectionError {
    InvalidPointCount {
        kind: DetectionAreaKind,
        expected: &'static str,
        actual: usize,
    },
    NonFinitePoint {
        index: usize,
    },
    DuplicatePoint {
        first: usize,
        second: usize,
    },
    DegeneratePolygon,
    NonPlanarPolygon,
    SelfIntersectingPolygon {
        first_edge: usize,
        second_edge: usize,
    },
    InvalidCube {
        reason: &'static str,
    },
}

impl fmt::Display for DetectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPointCount {
                kind,
                expected,
                actual,
            } => {
                write!(f, "{kind:?} requires {expected} points, got {actual}")?;
            }
            Self::NonFinitePoint { index } => {
                write!(f, "point[{index}] contains NaN or infinity")?;
            }
            Self::DuplicatePoint { first, second } => {
                write!(f, "point[{first}] and point[{second}] overlap")?;
            }
            Self::DegeneratePolygon => {
                write!(f, "polygon has no valid enclosed area")?;
            }
            Self::NonPlanarPolygon => {
                write!(f, "polygon points are not coplanar")?;
            }
            Self::SelfIntersectingPolygon {
                first_edge,
                second_edge,
            } => {
                write!(
                    f,
                    "polygon edges {first_edge} and {second_edge} intersect or overlap"
                )?;
            }
            Self::InvalidCube { reason } => {
                write!(f, "invalid cube: {reason}")?;
            }
        }

        Ok(())
    }
}

impl std::error::Error for DetectionError {}
