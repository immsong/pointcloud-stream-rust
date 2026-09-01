use base64::Engine;

use crate::pointcloud::{PointCloudFrame, PointFieldDataType};

const FOXGLOVE_BINARY_OP_MESSAGE_DATA: u8 = 0x01;

#[derive(serde::Serialize)]
struct FoxgloveTimestamp {
    sec: u64,
    nsec: u64,
}

#[derive(serde::Serialize)]
struct FoxgloveVector3 {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(serde::Serialize)]
struct FoxgloveQuaternion {
    x: f64,
    y: f64,
    z: f64,
    w: f64,
}

#[derive(serde::Serialize)]
struct FoxglovePose {
    position: FoxgloveVector3,
    orientation: FoxgloveQuaternion,
}

#[derive(serde::Serialize)]
struct FoxgloveField {
    name: String,
    offset: u32,

    #[serde(rename = "type")]
    data_type: u8,
}

#[derive(serde::Serialize)]
struct FoxglovePointCloud {
    timestamp: FoxgloveTimestamp,
    frame_id: String,
    pose: FoxglovePose,
    point_stride: u32,
    fields: Vec<FoxgloveField>,
    data: String,
}

fn foxglove_numeric_type(data_type: PointFieldDataType) -> u8 {
    match data_type {
        PointFieldDataType::Uint8 => 1,
        PointFieldDataType::Int8 => 2,
        PointFieldDataType::Uint16 => 3,
        PointFieldDataType::Int16 => 4,
        PointFieldDataType::Uint32 => 5,
        PointFieldDataType::Int32 => 6,
        PointFieldDataType::Float32 => 7,
        PointFieldDataType::Float64 => 8,
    }
}

fn convert_point_data_to_little_endian(data: &mut [u8], frame: &PointCloudFrame) {
    let point_count = frame.width as usize * frame.height as usize;

    let point_step = frame.point_step as usize;

    for point_index in 0..point_count {
        let point_start = point_index * point_step;

        for field in &frame.fields {
            let value_size = field.data_type.size_bytes();

            if value_size == 1 {
                // 데이터 1은 endian 영향을 받지 않음
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

fn encode_point_data(frame: &PointCloudFrame) -> Vec<u8> {
    let row_data_size = frame.width as usize * frame.point_step as usize;
    let mut data = Vec::with_capacity(row_data_size * frame.height as usize);

    for row in 0..frame.height as usize {
        let row_start = row * frame.row_step as usize;
        let row_end = row_start + row_data_size;

        data.extend_from_slice(&frame.data[row_start..row_end]);
    }

    if frame.endianness == crate::pointcloud::Endianness::Big {
        convert_point_data_to_little_endian(&mut data, frame);
    }

    data
}

pub fn encode_pointcloud_payload(frame: &PointCloudFrame) -> Result<Vec<u8>, serde_json::Error> {
    let mut fields: Vec<FoxgloveField> = vec![];
    for field in &frame.fields {
        fields.push(FoxgloveField {
            name: field.name.clone(),
            offset: field.offset,
            data_type: foxglove_numeric_type(field.data_type),
        });

        for i in 1..field.count {
            fields.push(FoxgloveField {
                name: format!("{}[{}]", field.name, i),
                offset: field.offset + i * field.data_type.size_bytes() as u32,
                data_type: foxglove_numeric_type(field.data_type),
            });
        }
    }

    let data = encode_point_data(frame);
    let foxgloves_pointcloud = FoxglovePointCloud {
        timestamp: FoxgloveTimestamp {
            sec: frame.timestamp_ns / 1_000_000_000,
            nsec: frame.timestamp_ns % 1_000_000_000,
        },
        frame_id: frame.frame_id.clone(),
        pose: FoxglovePose {
            position: FoxgloveVector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            orientation: FoxgloveQuaternion {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        },
        point_stride: frame.point_step,
        fields,
        data: base64::engine::general_purpose::STANDARD.encode(&data),
    };

    serde_json::to_vec(&foxgloves_pointcloud)
}

pub fn encode_message_data(subscription_id: u32, timestamp_ns: u64, payload: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(1 + 4 + 8 + payload.len());

    message.push(FOXGLOVE_BINARY_OP_MESSAGE_DATA);

    message.extend_from_slice(&subscription_id.to_le_bytes());

    message.extend_from_slice(&timestamp_ns.to_le_bytes());

    message.extend_from_slice(payload);

    message
}

#[test]
fn message_data_encodes_header() {
    let payload = [10, 20, 30];

    let message = encode_message_data(3, 1000, &payload);

    assert_eq!(message[0], 0x01);

    assert_eq!(&message[1..5], &3u32.to_le_bytes(),);

    assert_eq!(&message[5..13], &1000u64.to_le_bytes(),);

    assert_eq!(&message[13..], &payload,);
}

#[test]
fn pointcloud_payload_encodes_required_fields() {
    let mut data = Vec::new();
    data.extend_from_slice(&1.0f32.to_le_bytes());
    data.extend_from_slice(&2.0f32.to_le_bytes());
    data.extend_from_slice(&3.0f32.to_le_bytes());

    let frame = PointCloudFrame {
        timestamp_ns: 1_500_000_000,
        frame_id: "lidar".to_string(),
        width: 1,
        height: 1,
        fields: vec![
            crate::pointcloud::PointField {
                name: "x".to_string(),
                offset: 0,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "y".to_string(),
                offset: 4,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "z".to_string(),
                offset: 8,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
        ],
        endianness: crate::pointcloud::Endianness::Little,
        point_step: 12,
        row_step: 12,
        is_dense: true,
        data: data.clone(),
    };

    let payload = encode_pointcloud_payload(&frame).unwrap();

    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();

    assert_eq!(json["timestamp"]["sec"], 1);
    assert_eq!(json["timestamp"]["nsec"], 500_000_000);
    assert_eq!(json["frame_id"], "lidar");

    assert_eq!(json["pose"]["orientation"]["w"], 1.0);

    assert_eq!(json["point_stride"], 12);

    assert_eq!(json["fields"].as_array().unwrap().len(), 3);

    assert_eq!(json["fields"][0]["name"], "x");
    assert_eq!(json["fields"][0]["offset"], 0);
    assert_eq!(json["fields"][0]["type"], 7);

    assert_eq!(json["fields"][1]["name"], "y");
    assert_eq!(json["fields"][1]["offset"], 4);
    assert_eq!(json["fields"][1]["type"], 7);

    assert_eq!(json["fields"][2]["name"], "z");
    assert_eq!(json["fields"][2]["offset"], 8);
    assert_eq!(json["fields"][2]["type"], 7);

    let expected_data = base64::engine::general_purpose::STANDARD.encode(&data);

    assert_eq!(json["data"], expected_data);
}

#[test]
fn pointcloud_payload_removes_row_padding() {
    let mut data = Vec::new();

    // row 1 - point
    data.extend_from_slice(&1.0f32.to_le_bytes());
    data.extend_from_slice(&2.0f32.to_le_bytes());
    data.extend_from_slice(&3.0f32.to_le_bytes());

    // row padding
    data.extend_from_slice(&[99, 99, 99, 99]);

    // row 2 - point
    data.extend_from_slice(&4.0f32.to_le_bytes());
    data.extend_from_slice(&5.0f32.to_le_bytes());
    data.extend_from_slice(&6.0f32.to_le_bytes());

    // row padding
    data.extend_from_slice(&[88, 88, 88, 88]);

    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "lidar".to_string(),
        width: 1,
        height: 2,
        fields: vec![
            crate::pointcloud::PointField {
                name: "x".to_string(),
                offset: 0,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "y".to_string(),
                offset: 4,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "z".to_string(),
                offset: 8,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
        ],
        endianness: crate::pointcloud::Endianness::Little,
        point_step: 12,
        row_step: 16,
        is_dense: true,
        data,
    };

    let encoded = encode_point_data(&frame);

    assert_eq!(encoded.len(), 24);

    assert_eq!(
        &encoded[0..12],
        &[
            1.0f32.to_le_bytes(),
            2.0f32.to_le_bytes(),
            3.0f32.to_le_bytes(),
        ]
        .concat(),
    );

    assert_eq!(
        &encoded[12..24],
        &[
            4.0f32.to_le_bytes(),
            5.0f32.to_le_bytes(),
            6.0f32.to_le_bytes(),
        ]
        .concat(),
    );

    let payload = encode_pointcloud_payload(&frame).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();

    let encoded_data = json["data"].as_str().unwrap();

    let decoded_data = base64::engine::general_purpose::STANDARD
        .decode(encoded_data)
        .unwrap();

    assert_eq!(decoded_data.len(), 24);
}

#[test]
fn pointcloud_payload_converts_big_endian_to_little_endian() {
    let mut data = Vec::new();

    data.extend_from_slice(&1.0f32.to_be_bytes());
    data.extend_from_slice(&2.0f32.to_be_bytes());
    data.extend_from_slice(&3.0f32.to_be_bytes());

    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "lidar".to_string(),
        width: 1,
        height: 1,
        fields: vec![
            crate::pointcloud::PointField {
                name: "x".to_string(),
                offset: 0,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "y".to_string(),
                offset: 4,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "z".to_string(),
                offset: 8,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
        ],
        endianness: crate::pointcloud::Endianness::Big,
        point_step: 12,
        row_step: 12,
        is_dense: true,
        data,
    };

    let encoded = encode_point_data(&frame);

    let mut expected = Vec::new();
    expected.extend_from_slice(&1.0f32.to_le_bytes());
    expected.extend_from_slice(&2.0f32.to_le_bytes());
    expected.extend_from_slice(&3.0f32.to_le_bytes());

    assert_eq!(encoded, expected);
}

#[test]
fn pointcloud_payload_expands_multi_value_field() {
    let mut data = Vec::new();

    data.extend_from_slice(&1.0f32.to_le_bytes());
    data.extend_from_slice(&2.0f32.to_le_bytes());
    data.extend_from_slice(&3.0f32.to_le_bytes());

    data.extend_from_slice(&0.1f32.to_le_bytes());
    data.extend_from_slice(&0.2f32.to_le_bytes());
    data.extend_from_slice(&0.3f32.to_le_bytes());

    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "lidar".to_string(),
        width: 1,
        height: 1,
        fields: vec![
            crate::pointcloud::PointField {
                name: "x".to_string(),
                offset: 0,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "y".to_string(),
                offset: 4,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "z".to_string(),
                offset: 8,
                data_type: PointFieldDataType::Float32,
                count: 1,
            },
            crate::pointcloud::PointField {
                name: "normal".to_string(),
                offset: 12,
                data_type: PointFieldDataType::Float32,
                count: 3,
            },
        ],
        endianness: crate::pointcloud::Endianness::Little,
        point_step: 24,
        row_step: 24,
        is_dense: true,
        data,
    };

    let payload = encode_pointcloud_payload(&frame).unwrap();

    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();

    assert_eq!(json["fields"].as_array().unwrap().len(), 6);

    assert_eq!(json["fields"][3]["name"], "normal");
    assert_eq!(json["fields"][3]["offset"], 12);
    assert_eq!(json["fields"][3]["type"], 7);

    assert_eq!(json["fields"][4]["name"], "normal[1]");
    assert_eq!(json["fields"][4]["offset"], 16);
    assert_eq!(json["fields"][4]["type"], 7);

    assert_eq!(json["fields"][5]["name"], "normal[2]");
    assert_eq!(json["fields"][5]["offset"], 20);
    assert_eq!(json["fields"][5]["type"], 7);
}
