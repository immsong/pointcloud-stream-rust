/// 3D 좌표 이동값.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Translation {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// 3D 좌표 회전에 사용되는 Euler angle.
///
/// 각도는 degree 단위이며, 오른손 좌표계를 기준으로 한다.
///
/// 회전축:
/// - roll: X축
/// - pitch: Y축
/// - yaw: Z축
///
/// 적용 순서:
/// Roll(X) -> Pitch(Y) -> Yaw(Z)
///
/// 회전행렬:
/// R = Rz(yaw) * Ry(pitch) * Rx(roll)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotation {
    pub roll: f32,
    pub pitch: f32,
    pub yaw: f32,
}

/// 3D Point Cloud에 적용할 좌표 변환.
///
/// Rotation 적용 후 Translation을 적용한다.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    translation: Translation,

    sin_roll: f32,
    cos_roll: f32,
    sin_pitch: f32,
    cos_pitch: f32,
    sin_yaw: f32,
    cos_yaw: f32,
}

impl Transform {
    pub fn new(translation: Translation, rotation: Rotation) -> Self {
        let roll = rotation.roll.to_radians();
        let pitch = rotation.pitch.to_radians();
        let yaw = rotation.yaw.to_radians();

        let (sin_roll, cos_roll) = roll.sin_cos();
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();

        Self {
            translation,
            sin_roll,
            cos_roll,
            sin_pitch,
            cos_pitch,
            sin_yaw,
            cos_yaw,
        }
    }

    pub fn transform_point(&self, x: f32, y: f32, z: f32) -> (f32, f32, f32) {
        let transformed_x = x * self.cos_yaw * self.cos_pitch
            + y * (self.cos_yaw * self.sin_pitch * self.sin_roll - self.sin_yaw * self.cos_roll)
            + z * (self.cos_yaw * self.sin_pitch * self.cos_roll + self.sin_yaw * self.sin_roll)
            + self.translation.x;

        let transformed_y = x * self.sin_yaw * self.cos_pitch
            + y * (self.sin_yaw * self.sin_pitch * self.sin_roll + self.cos_yaw * self.cos_roll)
            + z * (self.sin_yaw * self.sin_pitch * self.cos_roll - self.cos_yaw * self.sin_roll)
            + self.translation.y;

        let transformed_z = -x * self.sin_pitch
            + y * self.cos_pitch * self.sin_roll
            + z * self.cos_pitch * self.cos_roll
            + self.translation.z;

        (transformed_x, transformed_y, transformed_z)
    }
}
