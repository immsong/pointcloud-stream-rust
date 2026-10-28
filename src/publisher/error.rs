use super::ChannelId;
use std::fmt;

#[derive(Debug)]
pub enum PublishError {
    UnknownChannel(ChannelId),
    WrongChannelKind(ChannelId),
    LengthOverflow(&'static str),
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChannel(id) => {
                write!(f, "unknown channel {}", id.as_u32())
            }
            Self::WrongChannelKind(id) => {
                write!(f, "channel {} is not correct kind of channel", id.as_u32())
            }
            Self::LengthOverflow(field) => {
                write!(f, "{field} exceeds wire u32 length")
            }
        }
    }
}
