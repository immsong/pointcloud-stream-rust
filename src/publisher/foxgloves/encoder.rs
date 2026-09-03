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
struct FoxglovePointCloud<'a> {
    timestamp: FoxgloveTimestamp,
    frame_id: String,
    pose: FoxglovePose,
    point_stride: u32,
    fields: Vec<FoxgloveField>,
    data: &'a [u8],
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

pub fn encode_foxglove_pointcloud_message(
    subscription_id: u32,
    timestamp_ns: u64,
    payload: &[u8],
) -> Vec<u8> {
    encode_message_data(subscription_id, timestamp_ns, payload)
}

pub fn encode_foxglove_pointcloud_payload(
    frame: &PointCloudFrame,
    point_data: &[u8],
) -> Result<Vec<u8>, serde_json::Error> {
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
        data: point_data,
    };

    serde_json::to_vec(&foxgloves_pointcloud)
}

fn encode_message_data(subscription_id: u32, timestamp_ns: u64, payload: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(1 + 4 + 8 + payload.len());

    message.push(FOXGLOVE_BINARY_OP_MESSAGE_DATA);

    message.extend_from_slice(&subscription_id.to_le_bytes());

    message.extend_from_slice(&timestamp_ns.to_le_bytes());

    message.extend_from_slice(payload);

    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pointcloud::{Endianness, PointField};

    #[test]
    fn message_data_encodes_header() {
        let payload = [10, 20, 30];

        let message = encode_message_data(3, 1000, &payload);

        assert_eq!(message[0], FOXGLOVE_BINARY_OP_MESSAGE_DATA);
        assert_eq!(&message[1..5], &3u32.to_le_bytes());
        assert_eq!(&message[5..13], &1000u64.to_le_bytes());
        assert_eq!(&message[13..], &payload);
    }

    #[test]
    fn pointcloud_payload_encodes_required_fields() {
        let mut point_data = Vec::new();

        point_data.extend_from_slice(&1.0f32.to_le_bytes());
        point_data.extend_from_slice(&2.0f32.to_le_bytes());
        point_data.extend_from_slice(&3.0f32.to_le_bytes());

        let frame = PointCloudFrame {
            timestamp_ns: 1_500_000_000,
            frame_id: "lidar".to_string(),
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
            endianness: Endianness::Little,
            point_step: 12,
            row_step: 12,
            is_dense: true,
            data: point_data.clone(),
        };

        let payload = encode_foxglove_pointcloud_payload(&frame, &point_data).unwrap();

        let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(json["timestamp"]["sec"], 1);
        assert_eq!(json["timestamp"]["nsec"], 500_000_000);
        assert_eq!(json["frame_id"], "lidar");

        assert_eq!(json["pose"]["position"]["x"], 0.0);
        assert_eq!(json["pose"]["position"]["y"], 0.0);
        assert_eq!(json["pose"]["position"]["z"], 0.0);

        assert_eq!(json["pose"]["orientation"]["x"], 0.0);
        assert_eq!(json["pose"]["orientation"]["y"], 0.0);
        assert_eq!(json["pose"]["orientation"]["z"], 0.0);
        assert_eq!(json["pose"]["orientation"]["w"], 1.0);

        assert_eq!(json["point_stride"], 12);

        let fields = json["fields"].as_array().unwrap();

        assert_eq!(fields.len(), 3);

        assert_eq!(fields[0]["name"], "x");
        assert_eq!(fields[0]["offset"], 0);
        assert_eq!(fields[0]["type"], 7);

        assert_eq!(fields[1]["name"], "y");
        assert_eq!(fields[1]["offset"], 4);
        assert_eq!(fields[1]["type"], 7);

        assert_eq!(fields[2]["name"], "z");
        assert_eq!(fields[2]["offset"], 8);
        assert_eq!(fields[2]["type"], 7);

        let actual_data: Vec<u8> = json["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| u8::try_from(value.as_u64().unwrap()).unwrap())
            .collect();

        assert_eq!(actual_data, point_data);
    }

    #[test]
    fn pointcloud_payload_expands_multi_value_field() {
        let fields = vec![
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
            PointField {
                name: "normal".to_string(),
                offset: 12,
                data_type: PointFieldDataType::Float32,
                count: 3,
            },
        ];

        let point_data = vec![0u8; 24];

        let frame = PointCloudFrame {
            timestamp_ns: 0,
            frame_id: "lidar".to_string(),
            width: 1,
            height: 1,
            fields,
            endianness: Endianness::Little,
            point_step: 24,
            row_step: 24,
            is_dense: true,
            data: point_data.clone(),
        };

        let payload = encode_foxglove_pointcloud_payload(&frame, &point_data).unwrap();

        let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();

        let fields = json["fields"].as_array().unwrap();

        assert_eq!(fields.len(), 6);

        assert_eq!(fields[3]["name"], "normal");
        assert_eq!(fields[3]["offset"], 12);
        assert_eq!(fields[3]["type"], 7);

        assert_eq!(fields[4]["name"], "normal[1]");
        assert_eq!(fields[4]["offset"], 16);
        assert_eq!(fields[4]["type"], 7);

        assert_eq!(fields[5]["name"], "normal[2]");
        assert_eq!(fields[5]["offset"], 20);
        assert_eq!(fields[5]["type"], 7);
    }

    #[test]
    fn foxglove_pointcloud_message_encodes_complete_message() {
        let point_data = [
            1.0f32.to_le_bytes(),
            2.0f32.to_le_bytes(),
            3.0f32.to_le_bytes(),
        ]
        .concat();

        let frame = PointCloudFrame {
            timestamp_ns: 123456789,
            frame_id: "lidar".to_string(),
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
            endianness: Endianness::Little,
            point_step: 12,
            row_step: 12,
            is_dense: true,
            data: point_data.clone(),
        };

        let subscription_id = 10;

        let payload = encode_foxglove_pointcloud_payload(&frame, &point_data);
        let message = encode_foxglove_pointcloud_message(
            subscription_id,
            frame.timestamp_ns,
            &payload.unwrap(),
        );

        assert_eq!(message[0], FOXGLOVE_BINARY_OP_MESSAGE_DATA);

        let actual_subscription_id = u32::from_le_bytes(message[1..5].try_into().unwrap());

        assert_eq!(actual_subscription_id, subscription_id);

        let actual_timestamp_ns = u64::from_le_bytes(message[5..13].try_into().unwrap());

        assert_eq!(actual_timestamp_ns, frame.timestamp_ns);

        let json: serde_json::Value = serde_json::from_slice(&message[13..]).unwrap();

        assert_eq!(json["frame_id"], "lidar");

        let actual_data: Vec<u8> = json["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| u8::try_from(value.as_u64().unwrap()).unwrap())
            .collect();

        assert_eq!(actual_data, point_data);
    }
}
