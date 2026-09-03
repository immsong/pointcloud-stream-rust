pub const WIRE_BINARY_OP_POINTCLOUD: u8 = 0x01;

pub fn encode_wire_pointcloud_message(
    subscription_id: u32,
    timestamp_ns: u64,
    point_data: &[u8],
) -> Vec<u8> {
    // 13 bytes for the header: 1 byte for operation, 4 bytes for subscription_id, 8 bytes for timestamp_ns
    let mut message = Vec::with_capacity(13 + point_data.len());

    message.push(WIRE_BINARY_OP_POINTCLOUD);
    message.extend_from_slice(&subscription_id.to_le_bytes());
    message.extend_from_slice(&timestamp_ns.to_le_bytes());
    message.extend_from_slice(point_data);

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointcloud_message_encodes_header_and_data() {
        let subscription_id = 10u32;
        let timestamp_ns = 123456789u64;

        let point_data = [
            1.0f32.to_le_bytes(),
            2.0f32.to_le_bytes(),
            3.0f32.to_le_bytes(),
        ]
        .concat();

        let message = encode_wire_pointcloud_message(subscription_id, timestamp_ns, &point_data);

        // 13-byte header + point data
        assert_eq!(message.len(), 13 + point_data.len());

        // opcode
        assert_eq!(message[0], WIRE_BINARY_OP_POINTCLOUD);

        // subscription_id
        let actual_subscription_id = u32::from_le_bytes(message[1..5].try_into().unwrap());

        assert_eq!(actual_subscription_id, subscription_id);

        // timestamp_ns
        let actual_timestamp_ns = u64::from_le_bytes(message[5..13].try_into().unwrap());

        assert_eq!(actual_timestamp_ns, timestamp_ns);

        // point data
        assert_eq!(&message[13..], point_data.as_slice());
    }

    #[test]
    fn pointcloud_message_preserves_point_data() {
        let point_data = vec![0, 1, 2, 3, 10, 20, 30, 40, 100, 200, 250];

        let message = encode_wire_pointcloud_message(1, 1000, &point_data);

        assert_eq!(&message[13..], point_data.as_slice());
    }
}
