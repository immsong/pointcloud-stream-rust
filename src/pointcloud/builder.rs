use crate::pointcloud::{
    Endianness, PointCloudError, PointCloudFrame, PointField, PointFieldDataType,
};

/// PointCloudFrame 생성을 단계적으로 구성하기 위한 Builder.
///
/// Field layout과 Frame 생성에 필요한 설정값을 수집하고,
/// 최종 `build()` 단계에서 `PointCloudFrame`을 생성한다.
#[derive(Debug)]
pub struct PointCloudBuilder {
    timestamp_ns: u64,
    frame_id: String,
    width: u32,
    height: u32,
    fields: Vec<PointField>,
    endianness: Endianness,
    is_dense: bool,

    // 일반적으로 fields에서 계산하지만,
    // padding이 필요한 경우 직접 지정할 수 있다.
    point_step: Option<u32>,

    // 일반적으로 width * point_step으로 계산하지만,
    // 행 단위 padding이 필요한 경우 직접 지정할 수 있다.
    row_step: Option<u32>,
}

impl PointCloudBuilder {
    pub fn new() -> Self {
        Self {
            timestamp_ns: 0,
            frame_id: String::new(),
            width: 0,
            height: 1,
            fields: Vec::new(),
            endianness: Endianness::Little,
            is_dense: true,
            point_step: None,
            row_step: None,
        }
    }

    pub fn timestamp_ns(mut self, timestamp_ns: u64) -> Self {
        self.timestamp_ns = timestamp_ns;
        self
    }

    pub fn frame_id(mut self, frame_id: impl Into<String>) -> Self {
        self.frame_id = frame_id.into();
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        self.height = height;
        self
    }

    pub fn fields(mut self, fields: Vec<PointField>) -> Self {
        self.fields = fields;
        self
    }

    pub fn field(
        mut self,
        name: impl Into<String>,
        data_type: PointFieldDataType,
        count: u32,
    ) -> Self {
        let next_offset = self
            .fields
            .iter()
            .max_by_key(|f| f.offset)
            .map_or(0, |f| f.offset + f.data_type.size_bytes() as u32 * f.count);

        self.fields.push(PointField {
            name: name.into(),
            offset: next_offset,
            data_type,
            count,
        });
        self
    }

    pub fn field_at(
        mut self,
        name: impl Into<String>,
        offset: u32,
        data_type: PointFieldDataType,
        count: u32,
    ) -> Self {
        self.fields.push(PointField {
            name: name.into(),
            offset,
            data_type,
            count,
        });
        self
    }

    pub fn endianness(mut self, endianness: Endianness) -> Self {
        self.endianness = endianness;
        self
    }

    pub fn is_dense(mut self, is_dense: bool) -> Self {
        self.is_dense = is_dense;
        self
    }

    pub fn point_step(mut self, point_step: u32) -> Self {
        self.point_step = Some(point_step);
        self
    }

    pub fn row_step(mut self, row_step: u32) -> Self {
        self.row_step = Some(row_step);
        self
    }

    pub fn build(self, data: Vec<u8>) -> Result<PointCloudFrame, PointCloudError> {
        let point_step = match self.point_step {
            Some(point_step) => point_step,
            None => {
                // offset이 가장 큰 field를 마지막 field로 판단하고,
                // 해당 field의 끝 위치를 기준으로 point_step을 계산한다.
                self.fields
                    .iter()
                    .max_by_key(|f| f.offset)
                    .map_or(0, |field| {
                        field.offset + field.data_type.size_bytes() as u32 * field.count
                    })
            }
        };

        let row_step = match self.row_step {
            Some(row_step) => row_step,
            None => self.width * point_step,
        };

        let frame = PointCloudFrame {
            timestamp_ns: self.timestamp_ns,
            frame_id: self.frame_id,
            width: self.width,
            height: self.height,
            fields: self.fields,
            endianness: self.endianness,
            point_step,
            row_step,
            is_dense: self.is_dense,
            data,
        };

        frame.validate()?;
        Ok(frame)
    }
}

impl Default for PointCloudBuilder {
    fn default() -> Self {
        Self::new()
    }
}
