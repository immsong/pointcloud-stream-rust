//! Wire 채널의 메시지 검증과 인코딩에 사용하는 내부 코덱 인터페이스.

use std::any::{Any, type_name};

use serde_json::{Map, Value};

use crate::publisher::PublishError;
use crate::publisher::protocols::wire::WireSchema;

/// 메시지 타입별 스키마와 검증, 인코딩을 정의하는 내부 인터페이스.
///
/// Point Cloud layout과 같은 채널별 설정은 코덱이 보관한다.
pub(crate) trait MessageCodec: Send + Sync + 'static {
    type Message: Send + Sync + 'static;

    fn schema(&self) -> WireSchema;

    /// 구독 응답에 포함할 추가 메타데이터를 반환한다.
    ///
    /// 프로토콜의 기본 필드와 같은 이름은 사용할 수 없다.
    fn subscription_metadata(&self) -> Map<String, Value> {
        Map::new()
    }

    fn timestamp_ns(&self, message: &Self::Message) -> u64;

    /// 입력 메시지를 변경하지 않고 유효성을 검증한다.
    ///
    /// 구독자가 없는 경우에도 호출된다.
    fn validate(&self, _message: &Self::Message) -> Result<(), PublishError> {
        Ok(())
    }

    /// 검증된 메시지를 binary payload로 인코딩한다.
    ///
    /// 구독자가 있는 발행 요청에서 `validate()` 성공 후 한 번 호출된다.
    /// 직접 호출하는 경우에도 먼저 `validate()`로 검증해야 한다.
    fn encode(&self, message: &Self::Message) -> Result<Vec<u8>, PublishError>;
}

/// 서로 다른 메시지 타입의 코덱을 같은 채널 목록에 보관하기 위한 인터페이스.
pub(crate) trait ErasedCodec: Send + Sync {
    fn timestamp_ns(&self, message: &dyn Any, actual: &'static str) -> Result<u64, PublishError>;

    fn validate(&self, message: &dyn Any, actual: &'static str) -> Result<(), PublishError>;

    fn encode(&self, message: &dyn Any, actual: &'static str) -> Result<Vec<u8>, PublishError>;
}

/// `Any`로 전달된 메시지를 코덱의 메시지 타입으로 변환하는 어댑터.
pub(crate) struct CodecAdapter<C>(pub C);

impl<C: MessageCodec> CodecAdapter<C> {
    fn message<'a>(
        &self,
        message: &'a dyn Any,
        actual: &'static str,
    ) -> Result<&'a C::Message, PublishError> {
        message
            .downcast_ref::<C::Message>()
            .ok_or(PublishError::MessageTypeMismatch {
                expected: type_name::<C::Message>(),
                actual,
            })
    }
}

impl<C: MessageCodec> ErasedCodec for CodecAdapter<C> {
    fn timestamp_ns(&self, message: &dyn Any, actual: &'static str) -> Result<u64, PublishError> {
        Ok(self.0.timestamp_ns(self.message(message, actual)?))
    }

    fn validate(&self, message: &dyn Any, actual: &'static str) -> Result<(), PublishError> {
        self.0.validate(self.message(message, actual)?)
    }

    fn encode(&self, message: &dyn Any, actual: &'static str) -> Result<Vec<u8>, PublishError> {
        self.0.encode(self.message(message, actual)?)
    }
}
