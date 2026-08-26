use pointcloud_stream::pointcloud::{Rotation, Transform, Translation};

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
