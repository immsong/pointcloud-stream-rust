use pointcloud_stream::pointcloud::{
    Endianness, PointCloudError, PointCloudFrame, PointField, PointFieldDataType,
};

#[test]
fn valid_frame_passes_validation() {
    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "lidar".to_string(),
        width: 2,
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
        row_step: 24,
        is_dense: true,
        data: vec![0; 24],
    };

    assert!(frame.validate().is_ok());
}

#[test]
fn valid_frame_fails_validation() {
    let frame = PointCloudFrame {
        timestamp_ns: 0,
        frame_id: "lidar".to_string(),
        width: 2,
        height: 5,
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
        row_step: 24,
        is_dense: true,
        data: vec![0; 24],
    };

    assert_eq!(frame.validate(), Err(PointCloudError::InvalidDataLength));
}
