use pointcloud_stream::pointcloud::{PointCloudBuilder, PointFieldDataType};

#[test]
fn builder_valid_config_creates_frame() {
    let data = vec![0; 24];

    let frame = PointCloudBuilder::new()
        .frame_id("lidar")
        .width(2)
        .height(1)
        .field("x", PointFieldDataType::Float32, 1)
        .field("y", PointFieldDataType::Float32, 1)
        .field("z", PointFieldDataType::Float32, 1)
        .build(data)
        .unwrap();

    assert_eq!(frame.point_step, 12);
    assert_eq!(frame.row_step, 24);

    assert_eq!(frame.fields[0].offset, 0);
    assert_eq!(frame.fields[1].offset, 4);
    assert_eq!(frame.fields[2].offset, 8);
}

#[test]
fn builder_custom_field_offset_creates_valid_frame() {
    let data = vec![0; 32];

    let frame = PointCloudBuilder::new()
        .width(2)
        .height(1)
        .field_at("x", 0, PointFieldDataType::Float32, 1)
        .field_at("y", 8, PointFieldDataType::Float32, 1)
        .field_at("z", 12, PointFieldDataType::Float32, 1)
        .build(data)
        .unwrap();

    assert_eq!(frame.fields[0].offset, 0);
    assert_eq!(frame.fields[1].offset, 8);
    assert_eq!(frame.fields[2].offset, 12);

    assert_eq!(frame.point_step, 16);
    assert_eq!(frame.row_step, 32);
}

#[test]
fn builder_step_override_creates_valid_frame() {
    let data = vec![0; 64];

    let frame = PointCloudBuilder::new()
        .width(2)
        .height(1)
        .field("x", PointFieldDataType::Float32, 1)
        .field("y", PointFieldDataType::Float32, 1)
        .field("z", PointFieldDataType::Float32, 1)
        .point_step(16)
        .row_step(64)
        .build(data)
        .unwrap();

    assert_eq!(frame.point_step, 16);
    assert_eq!(frame.row_step, 64);
}
