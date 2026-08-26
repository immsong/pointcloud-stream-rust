use crate::pointcloud::{Endianness, PointCloudError, PointCloudFrame};

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

    pub fn transform_frame(&self, frame: &mut PointCloudFrame) -> Result<(), PointCloudError> {
        frame.validate()?;

        let x_offset = frame
            .fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case("x"))
            .unwrap()
            .offset as usize;
        let y_offset = frame
            .fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case("y"))
            .unwrap()
            .offset as usize;
        let z_offset = frame
            .fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case("z"))
            .unwrap()
            .offset as usize;

        for row in 0..frame.height as usize {
            for column in 0..frame.width as usize {
                let offset = (row * frame.row_step as usize) + (column * frame.point_step as usize);

                let x = Self::read_f32(&frame.data, offset + x_offset, frame.endianness);
                let y = Self::read_f32(&frame.data, offset + y_offset, frame.endianness);
                let z = Self::read_f32(&frame.data, offset + z_offset, frame.endianness);

                let (transformed_x, transformed_y, transformed_z) = self.transform_point(x, y, z);

                Self::write_f32(
                    &mut frame.data,
                    offset + x_offset,
                    transformed_x,
                    frame.endianness,
                );
                Self::write_f32(
                    &mut frame.data,
                    offset + y_offset,
                    transformed_y,
                    frame.endianness,
                );
                Self::write_f32(
                    &mut frame.data,
                    offset + z_offset,
                    transformed_z,
                    frame.endianness,
                );
            }
        }

        Ok(())
    }

    fn read_f32(data: &[u8], offset: usize, endianness: Endianness) -> f32 {
        let bytes: [u8; 4] = data[offset..offset + 4].try_into().unwrap();
        match endianness {
            Endianness::Little => f32::from_le_bytes(bytes),
            Endianness::Big => f32::from_be_bytes(bytes),
        }
    }

    fn write_f32(data: &mut [u8], offset: usize, value: f32, endianness: Endianness) {
        let bytes = match endianness {
            Endianness::Little => value.to_le_bytes(),
            Endianness::Big => value.to_be_bytes(),
        };
        data[offset..offset + 4].copy_from_slice(&bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_f32_read_write_preserves_value() {
        const EPSILON: f32 = 0.0001;

        let mut little_data = vec![0u8; 4];

        Transform::write_f32(&mut little_data, 0, 1.25, Endianness::Little);

        let little_value = Transform::read_f32(&little_data, 0, Endianness::Little);

        assert!((little_value - 1.25).abs() < EPSILON);

        let mut big_data = vec![0u8; 4];

        Transform::write_f32(&mut big_data, 0, 2.5, Endianness::Big);

        let big_value = Transform::read_f32(&big_data, 0, Endianness::Big);

        assert!((big_value - 2.5).abs() < EPSILON);
    }
}
