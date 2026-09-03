use crate::pointcloud::{Endianness, PointCloudFrame};

pub const WIRE_BINARY_OP_POINTCLOUD: u8 = 0x01;

pub fn encode_pointcloud_message(subscription_id: u32, frame: &PointCloudFrame) -> Vec<u8> {
    let point_data = encode_point_data(frame);

    // 13 bytes for the header: 1 byte for operation, 4 bytes for subscription_id, 8 bytes for timestamp_ns
    let mut message = Vec::with_capacity(13 + point_data.len());

    message.push(WIRE_BINARY_OP_POINTCLOUD);
    message.extend_from_slice(&subscription_id.to_le_bytes());
    message.extend_from_slice(&frame.timestamp_ns.to_le_bytes());
    message.extend_from_slice(&point_data);

    message
}

fn encode_point_data(frame: &PointCloudFrame) -> Vec<u8> {
    let point_step = frame.point_step as usize;
    let row_step = frame.row_step as usize;
    let width = frame.width as usize;
    let height = frame.height as usize;

    let row_data_size = width * point_step;

    let mut data = Vec::with_capacity(row_data_size * height);

    // row padding을 제외하고 실제 point data만 복사한다.
    for row in 0..height {
        let row_start = row * row_step;
        let row_end = row_start + row_data_size;

        data.extend_from_slice(&frame.data[row_start..row_end]);
    }

    if frame.endianness == Endianness::Big {
        convert_point_data_to_little_endian(&mut data, frame);
    }

    data
}

fn convert_point_data_to_little_endian(data: &mut [u8], frame: &PointCloudFrame) {
    let point_count = frame.width as usize * frame.height as usize;

    let point_step = frame.point_step as usize;

    for point_index in 0..point_count {
        let point_start = point_index * point_step;

        for field in &frame.fields {
            let value_size = field.data_type.size_bytes();

            if value_size == 1 {
                // 1바이트 데이터는 endian 영향을 받지 않음.
                continue;
            }

            for value_index in 0..field.count as usize {
                let value_start = point_start + field.offset as usize + value_index * value_size;
                let value_end = value_start + value_size;

                data[value_start..value_end].reverse();
            }
        }
    }
}

#[test]
fn removes_row_padding() {
    use crate::pointcloud::{PointField, PointFieldDataType};

    let first = 1.0f32.to_le_bytes();
    let second = 2.0f32.to_le_bytes();

    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "map".to_string(),
        width: 1,
        height: 2,
        fields: vec![PointField {
            name: "x".to_string(),
            offset: 0,
            data_type: PointFieldDataType::Float32,
            count: 1,
        }],
        endianness: Endianness::Little,
        point_step: 4,

        // 실제 point data는 4바이트이고,
        // 각 row 뒤에 4바이트 padding이 존재한다.
        row_step: 8,

        is_dense: true,
        data: vec![
            // row 0
            first[0], first[1], first[2], first[3], 0, 0, 0, 0, // row 1
            second[0], second[1], second[2], second[3], 0, 0, 0, 0,
        ],
    };

    let message = encode_pointcloud_message(1, &frame);

    // header 13 bytes + point data 8 bytes
    assert_eq!(message.len(), 21);

    // row padding이 제거되고 point data만 연속으로 들어가야 한다.
    assert_eq!(&message[13..17], &first);
    assert_eq!(&message[17..21], &second);
}

#[test]
fn converts_big_endian_point_data_to_little_endian() {
    use crate::pointcloud::{PointField, PointFieldDataType};

    let x = 1.5f32;
    let y = 2.5f32;
    let z = 3.5f32;

    let mut data = Vec::new();

    data.extend_from_slice(&x.to_be_bytes());
    data.extend_from_slice(&y.to_be_bytes());
    data.extend_from_slice(&z.to_be_bytes());

    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "map".to_string(),
        width: 1,
        height: 1,
        fields: vec![
            PointField {
                name: "x".to_string(),
                offset: 0,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            PointField {
                name: "y".to_string(),
                offset: 4,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            PointField {
                name: "z".to_string(),
                offset: 8,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
        ],
        endianness: Endianness::Big,
        point_step: 12,
        row_step: 12,
        is_dense: true,
        data,
    };

    let message = encode_pointcloud_message(1, &frame);

    let point_data = &message[13..];

    assert_eq!(&point_data[0..4], &x.to_le_bytes());
    assert_eq!(&point_data[4..8], &y.to_le_bytes());
    assert_eq!(&point_data[8..12], &z.to_le_bytes());
}

#[test]
fn converts_each_counted_value_to_little_endian() {
    use crate::pointcloud::{PointField, PointFieldDataType};

    let values = [1.0f32, 2.0f32, 3.0f32];

    let mut data = Vec::new();

    for value in values {
        data.extend_from_slice(&value.to_be_bytes());
    }

    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "map".to_string(),
        width: 1,
        height: 1,
        fields: vec![PointField {
            name: "normal".to_string(),
            offset: 0,
            data_type: PointFieldDataType::Float32,
            count: 3,
        }],
        endianness: Endianness::Big,
        point_step: 12,
        row_step: 12,
        is_dense: true,
        data,
    };

    let message = encode_pointcloud_message(1, &frame);

    let point_data = &message[13..];

    assert_eq!(&point_data[0..4], &1.0f32.to_le_bytes());
    assert_eq!(&point_data[4..8], &2.0f32.to_le_bytes());
    assert_eq!(&point_data[8..12], &3.0f32.to_le_bytes());
}
