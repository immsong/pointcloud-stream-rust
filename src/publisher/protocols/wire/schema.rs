/// 채널 목록과 구독 응답으로 전달하는 메시지 스키마 정보.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireSchema {
    pub schema_name: String,
    pub schema_version: u32,
    pub encoding: String,
}

impl WireSchema {
    pub fn new(name: impl Into<String>, version: u32, encoding: impl Into<String>) -> Self {
        Self {
            schema_name: name.into(),
            schema_version: version,
            encoding: encoding.into(),
        }
    }
}
