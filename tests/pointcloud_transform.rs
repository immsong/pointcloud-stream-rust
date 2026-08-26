use pointcloud_stream::pointcloud::{
    PointCloudBuilder, PointFieldDataType, Rotation, Transform, Translation,
};

#[test]
fn transform_point_applies_rotation_and_translation() {
    const EPSILON: f32 = 0.0001;

    let transform = Transform::new(
        Translation {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        },
        Rotation {
            roll: 0.0,
            pitch: 0.0,
            yaw: 90.0,
        },
    );

    let (x, y, z) = transform.transform_point(1.0, 0.0, 1.0);

    assert!((x - 1.0).abs() < EPSILON);
    assert!((y - 3.0).abs() < EPSILON);
    assert!((z - 4.0).abs() < EPSILON);
}

#[test]
fn transform_point_applies_roll_pitch_yaw_rotation() {
    const EPSILON: f32 = 0.0001;

    let roll = Transform::new(
        Translation {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        Rotation {
            roll: 90.0,
            pitch: 0.0,
            yaw: 0.0,
        },
    );

    let (x, y, z) = roll.transform_point(0.0, 1.0, 0.0);

    assert!((x - 0.0).abs() < EPSILON);
    assert!((y - 0.0).abs() < EPSILON);
    assert!((z - 1.0).abs() < EPSILON);

    let pitch = Transform::new(
        Translation {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        Rotation {
            roll: 0.0,
            pitch: 90.0,
            yaw: 0.0,
        },
    );

    let (x, y, z) = pitch.transform_point(1.0, 0.0, 0.0);

    assert!((x - 0.0).abs() < EPSILON);
    assert!((y - 0.0).abs() < EPSILON);
    assert!((z + 1.0).abs() < EPSILON);

    let yaw = Transform::new(
        Translation {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        Rotation {
            roll: 0.0,
            pitch: 0.0,
            yaw: 90.0,
        },
    );

    let (x, y, z) = yaw.transform_point(1.0, 0.0, 0.0);

    assert!((x - 0.0).abs() < EPSILON);
    assert!((y - 1.0).abs() < EPSILON);
    assert!((z - 0.0).abs() < EPSILON);
}

#[test]
fn transform_frame_applies_transform_to_all_points() {
    const EPSILON: f32 = 0.0001;

    let mut data = Vec::new();

    // point 1: x=1, y=2, z=3, intensity=10
    data.extend_from_slice(&1.0_f32.to_le_bytes());
    data.extend_from_slice(&2.0_f32.to_le_bytes());
    data.extend_from_slice(&3.0_f32.to_le_bytes());
    data.extend_from_slice(&10.0_f32.to_le_bytes());

    // point 2: x=4, y=5, z=6, intensity=20
    data.extend_from_slice(&4.0_f32.to_le_bytes());
    data.extend_from_slice(&5.0_f32.to_le_bytes());
    data.extend_from_slice(&6.0_f32.to_le_bytes());
    data.extend_from_slice(&20.0_f32.to_le_bytes());

    let mut frame = PointCloudBuilder::new()
        .width(2)
        .height(1)
        .field("x", PointFieldDataType::Float32, 1)
        .field("y", PointFieldDataType::Float32, 1)
        .field("z", PointFieldDataType::Float32, 1)
        .field("intensity", PointFieldDataType::Float32, 1)
        .build(data)
        .unwrap();

    let transform = Transform::new(
        Translation {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        },
        Rotation {
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
        },
    );

    transform.transform_frame(&mut frame).unwrap();

    let x1 = f32::from_le_bytes(frame.data[0..4].try_into().unwrap());
    let y1 = f32::from_le_bytes(frame.data[4..8].try_into().unwrap());
    let z1 = f32::from_le_bytes(frame.data[8..12].try_into().unwrap());
    let intensity1 = f32::from_le_bytes(frame.data[12..16].try_into().unwrap());

    let x2 = f32::from_le_bytes(frame.data[16..20].try_into().unwrap());
    let y2 = f32::from_le_bytes(frame.data[20..24].try_into().unwrap());
    let z2 = f32::from_le_bytes(frame.data[24..28].try_into().unwrap());
    let intensity2 = f32::from_le_bytes(frame.data[28..32].try_into().unwrap());

    assert!((x1 - 11.0).abs() < EPSILON);
    assert!((y1 - 22.0).abs() < EPSILON);
    assert!((z1 - 33.0).abs() < EPSILON);
    assert!((intensity1 - 10.0).abs() < EPSILON);

    assert!((x2 - 14.0).abs() < EPSILON);
    assert!((y2 - 25.0).abs() < EPSILON);
    assert!((z2 - 36.0).abs() < EPSILON);
    assert!((intensity2 - 20.0).abs() < EPSILON);
}
