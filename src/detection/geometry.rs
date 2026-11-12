//! 감지 영역의 형상 검증 및 꼭짓점 순서 정규화.
//!
//! `Polygon2D`의 꼭짓점은 외곽선 순서로 전달해야 한다.
//! 꼭짓점만으로 연결 순서를 결정할 수 없으므로, 교차한 다각형은 오류로 처리한다.
//!
//! `Cube3D`는 회전된 형상을 포함한 직육면체를 지원한다.
//! 8개 꼭짓점의 입력 순서와 관계없이 오른손 좌표계의 P0~P7 순서로 정규화한다.

use std::cmp::Ordering;

use crate::detection::{DetectionAreaKind, DetectionError};

type Vector2 = [f64; 2];
type Vector3 = [f64; 3];

// Float32 좌표의 연산 오차를 고려해 형상 검증에 사용할 허용 오차를 정의한다.
const RELATIVE_TOLERANCE: f64 = 1.0e-4;
const ABSOLUTE_TOLERANCE: f64 = 1.0e-7;

#[inline]
fn to_vector3(point: [f32; 3]) -> Vector3 {
    [point[0] as f64, point[1] as f64, point[2] as f64]
}

#[inline]
fn add(a: Vector3, b: Vector3) -> Vector3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn sub(a: Vector3, b: Vector3) -> Vector3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn dot(a: Vector3, b: Vector3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn cross(a: Vector3, b: Vector3) -> Vector3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn norm(a: Vector3) -> f64 {
    dot(a, a).sqrt()
}

#[inline]
fn sub2(a: Vector2, b: Vector2) -> Vector2 {
    [a[0] - b[0], a[1] - b[1]]
}

#[inline]
fn dot2(a: Vector2, b: Vector2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

#[inline]
fn cross2(a: Vector2, b: Vector2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

#[inline]
fn norm2(a: Vector2) -> f64 {
    dot2(a, a).sqrt()
}

#[inline]
fn tolerance(scale: f64) -> f64 {
    (scale * RELATIVE_TOLERANCE).max(ABSOLUTE_TOLERANCE)
}

fn finite_points(points: &[[f32; 3]]) -> Result<Vec<Vector3>, DetectionError> {
    let mut result = Vec::with_capacity(points.len());

    for (index, point) in points.iter().enumerate() {
        if !point.iter().all(|value| value.is_finite()) {
            return Err(DetectionError::NonFinitePoint { index });
        }

        result.push(to_vector3(*point));
    }

    Ok(result)
}

fn check_duplicates(points: &[Vector3], epsilon: f64) -> Result<(), DetectionError> {
    for i in 0..points.len() {
        for j in (i + 1)..points.len() {
            if norm(sub(points[i], points[j])) <= epsilon {
                return Err(DetectionError::DuplicatePoint {
                    first: i,
                    second: j,
                });
            }
        }
    }

    Ok(())
}

fn bounds_diagonal(points: &[Vector3]) -> f64 {
    let mut min = points[0];
    let mut max = points[0];

    for point in points.iter().skip(1) {
        for k in 0..3 {
            min[k] = min[k].min(point[k]);
            max[k] = max[k].max(point[k]);
        }
    }

    norm(sub(max, min))
}

#[inline]
fn project(local_point: Vector3, dropped_axis: usize) -> Vector2 {
    // 오른손 좌표계 기준으로 투영한다: +X -> (Y, Z), +Y -> (Z, X), +Z -> (X, Y).
    match dropped_axis {
        0 => [local_point[1], local_point[2]],
        1 => [local_point[2], local_point[0]],
        _ => [local_point[0], local_point[1]],
    }
}

fn orientation(a: Vector2, b: Vector2, c: Vector2, epsilon: f64) -> i8 {
    let ab = sub2(b, a);
    let ac = sub2(c, a);
    let determinant = cross2(ab, ac);
    let limit = epsilon * (norm2(ab) + norm2(ac));

    if determinant > limit {
        1
    } else if determinant < -limit {
        -1
    } else {
        0
    }
}

fn on_segment(a: Vector2, b: Vector2, point: Vector2, epsilon: f64) -> bool {
    point[0] >= a[0].min(b[0]) - epsilon
        && point[0] <= a[0].max(b[0]) + epsilon
        && point[1] >= a[1].min(b[1]) - epsilon
        && point[1] <= a[1].max(b[1]) + epsilon
}

fn segments_intersect(a: Vector2, b: Vector2, c: Vector2, d: Vector2, epsilon: f64) -> bool {
    let o1 = orientation(a, b, c, epsilon);
    let o2 = orientation(a, b, d, epsilon);
    let o3 = orientation(c, d, a, epsilon);
    let o4 = orientation(c, d, b, epsilon);

    if o1 != 0 && o2 != 0 && o3 != 0 && o4 != 0 && o1 != o2 && o3 != o4 {
        return true;
    }

    (o1 == 0 && on_segment(a, b, c, epsilon))
        || (o2 == 0 && on_segment(a, b, d, epsilon))
        || (o3 == 0 && on_segment(c, d, a, epsilon))
        || (o4 == 0 && on_segment(c, d, b, epsilon))
}

/// 다각형을 검증하고 투영 평면에서의 부호 있는 면적을 반환한다.
///
/// 투영 축은 평면 법선에서 선택하며, 면적이 양수이면 반시계 방향(CCW)이다.
fn polygon_signed_area(points: &[[f32; 3]]) -> Result<f64, DetectionError> {
    if points.len() < 3 {
        return Err(DetectionError::InvalidPointCount {
            kind: DetectionAreaKind::Polygon2D,
            expected: "at least 3",
            actual: points.len(),
        });
    }

    let coordinates = finite_points(points)?;
    let scale = bounds_diagonal(&coordinates);
    let epsilon = tolerance(scale);

    check_duplicates(&coordinates, epsilon)?;

    let origin = coordinates[0];

    // 첫 꼭짓점에서 가장 먼 점을 선택해 평면 계산의 기준선을 만든다.
    let mut farthest_index = 1usize;

    for i in 2..coordinates.len() {
        if norm(sub(coordinates[i], origin)) > norm(sub(coordinates[farthest_index], origin)) {
            farthest_index = i;
        }
    }

    let baseline = sub(coordinates[farthest_index], origin);
    let baseline_length = norm(baseline);

    if baseline_length <= epsilon {
        return Err(DetectionError::DegeneratePolygon);
    }

    // 기준선에서 가장 먼 점을 사용해 평면의 법선을 구한다.
    let mut normal = [0.0; 3];

    for point in &coordinates {
        let candidate = cross(baseline, sub(*point, origin));

        if norm(candidate) > norm(normal) {
            normal = candidate;
        }
    }

    let normal_length = norm(normal);

    if normal_length <= epsilon * baseline_length {
        return Err(DetectionError::DegeneratePolygon);
    }

    for point in &coordinates {
        let distance_to_plane = dot(sub(*point, origin), normal).abs() / normal_length;

        if distance_to_plane > epsilon {
            return Err(DetectionError::NonPlanarPolygon);
        }
    }

    let dropped_axis = if normal[0].abs() >= normal[1].abs() && normal[0].abs() >= normal[2].abs() {
        0
    } else if normal[1].abs() >= normal[2].abs() {
        1
    } else {
        2
    };

    // 기준점 P0을 빼서 큰 절대좌표에서의 면적 계산 오차를 줄인다.
    let projected: Vec<Vector2> = coordinates
        .iter()
        .map(|point| project(sub(*point, origin), dropped_axis))
        .collect();
    let point_count = projected.len();

    for i in 0..point_count {
        let previous_edge = (i + point_count - 1) % point_count;
        let next_edge = i;
        let previous = sub2(projected[i], projected[previous_edge]);
        let next = sub2(projected[(i + 1) % point_count], projected[i]);

        if norm2(next) <= epsilon {
            return Err(DetectionError::DegeneratePolygon);
        }

        // 같은 직선 위의 인접한 선분이 반대 방향으로 이어지면 겹침으로 처리한다.
        if cross2(previous, next).abs() <= epsilon * (norm2(previous) + norm2(next))
            && dot2(previous, next) < 0.0
        {
            return Err(DetectionError::SelfIntersectingPolygon {
                first_edge: previous_edge,
                second_edge: next_edge,
            });
        }
    }

    // 서로 인접하지 않은 선분의 교차, 접촉 및 겹침을 검사한다.
    for i in 0..point_count {
        for j in (i + 1)..point_count {
            if j == i + 1 || (i == 0 && j == point_count - 1) {
                continue;
            }

            if segments_intersect(
                projected[i],
                projected[(i + 1) % point_count],
                projected[j],
                projected[(j + 1) % point_count],
                epsilon,
            ) {
                return Err(DetectionError::SelfIntersectingPolygon {
                    first_edge: i,
                    second_edge: j,
                });
            }
        }
    }

    let twice_area: f64 = (0..point_count)
        .map(|i| cross2(projected[i], projected[(i + 1) % point_count]))
        .sum();

    if twice_area.abs() <= 2.0 * epsilon * scale {
        return Err(DetectionError::DegeneratePolygon);
    }

    Ok(twice_area * 0.5)
}

pub(super) fn validate_polygon(points: &[[f32; 3]]) -> Result<(), DetectionError> {
    polygon_signed_area(points).map(|_| ())
}

pub(super) fn normalize_polygon(points: &[[f32; 3]]) -> Result<Vec<[f32; 3]>, DetectionError> {
    let signed_area = polygon_signed_area(points)?;
    let mut normalized = points.to_vec();

    if signed_area < 0.0 {
        // 첫 꼭짓점은 고정하고 나머지 순서만 뒤집어 반시계 방향으로 맞춘다.
        normalized[1..].reverse();
    }

    Ok(normalized)
}

/// 직육면체를 검증하고 P0~P7 순서에 해당하는 입력 꼭짓점의 인덱스를 반환한다.
///
/// 회전된 직육면체도 지원하며, 원본 좌표값은 변경하지 않는다.
fn cube_indices(points: &[[f32; 3]]) -> Result<[usize; 8], DetectionError> {
    if points.len() != 8 {
        return Err(DetectionError::InvalidPointCount {
            kind: DetectionAreaKind::Cube3D,
            expected: "exactly 8",
            actual: points.len(),
        });
    }

    let coordinates = finite_points(points)?;

    // X, Y, Z 좌표를 차례로 비교해 입력 순서와 무관한 시작점을 선택한다.
    let origin_index = (0..8)
        .min_by(|&a, &b| lex_order(coordinates[a], coordinates[b]))
        .expect("cuboid contains 8 points");

    let origin = coordinates[origin_index];

    // 입력 순서와 관계없이 동일한 순서로 후보를 탐색한다.
    let mut neighbor_indices: Vec<usize> = (0..8).filter(|&i| i != origin_index).collect();

    neighbor_indices.sort_by(|&a, &b| lex_order(coordinates[a], coordinates[b]));

    let shortest = neighbor_indices
        .iter()
        .map(|&i| norm(sub(coordinates[i], origin)))
        .fold(f64::INFINITY, f64::min);

    // 긴 변 때문에 짧은 변의 허용 오차가 과도하게 커지지 않도록 한다.
    let epsilon = tolerance(shortest);
    check_duplicates(&coordinates, epsilon)?;

    if shortest <= epsilon {
        return Err(DetectionError::InvalidCube {
            reason: "cuboid edges must have positive length",
        });
    }

    // 시작점을 제외한 7개 꼭짓점에서 모서리 후보 3개를 선택한다.
    for i in 0..neighbor_indices.len() - 2 {
        for j in i + 1..neighbor_indices.len() - 1 {
            for k in j + 1..neighbor_indices.len() {
                let mut edge_indices = [
                    neighbor_indices[i],
                    neighbor_indices[j],
                    neighbor_indices[k],
                ];

                let edges = edge_indices.map(|id| sub(coordinates[id], origin));
                let lengths = edges.map(norm);

                if lengths.iter().any(|&length| length <= epsilon) {
                    continue;
                }

                // 세 모서리는 서로 직교해야 한다.
                let perpendicular = [(0usize, 1usize), (1, 2), (2, 0)].iter().all(|&(a, b)| {
                    let limit = 3.0 * epsilon * lengths[a].max(lengths[b]);

                    dot(edges[a], edges[b]).abs() <= limit
                });

                if !perpendicular {
                    continue;
                }

                // 모서리 길이의 영향을 받지 않도록 단위 방향으로 축 우선순위를 결정한다.
                edge_indices.sort_by(|&a, &b| {
                    let direction_a = sub(coordinates[a], origin);
                    let direction_b = sub(coordinates[b], origin);
                    let length_a = norm(direction_a);
                    let length_b = norm(direction_b);

                    (direction_b[0].abs() / length_b)
                        .total_cmp(&(direction_a[0].abs() / length_a))
                        .then_with(|| {
                            (direction_b[1].abs() / length_b)
                                .total_cmp(&(direction_a[1].abs() / length_a))
                        })
                        .then_with(|| {
                            (direction_b[2].abs() / length_b)
                                .total_cmp(&(direction_a[2].abs() / length_a))
                        })
                        .then_with(|| lex_order(coordinates[a], coordinates[b]))
                });

                let first_edge = sub(coordinates[edge_indices[0]], origin);
                let mut second_edge = sub(coordinates[edge_indices[1]], origin);
                let mut third_edge = sub(coordinates[edge_indices[2]], origin);

                // 세 모서리가 오른손 좌표계를 이루도록 순서를 맞춘다.
                if dot(cross(first_edge, second_edge), third_edge) < 0.0 {
                    std::mem::swap(&mut second_edge, &mut third_edge);
                }

                // P0~P3은 한 면을 이루며, P4~P7은 세 번째 모서리만큼 이동한 반대쪽 면이다.
                let expected = [
                    origin,
                    add(origin, first_edge),
                    add(add(origin, first_edge), second_edge),
                    add(origin, second_edge),
                    add(origin, third_edge),
                    add(add(origin, first_edge), third_edge),
                    add(add(add(origin, first_edge), second_edge), third_edge),
                    add(add(origin, second_edge), third_edge),
                ];

                let mut used = [false; 8];
                let mut ordered_indices = [0usize; 8];
                let mut matched = true;

                for (vertex_index, target) in expected.iter().enumerate() {
                    let best = (0..8)
                        .filter(|&id| !used[id])
                        .map(|id| (id, norm(sub(coordinates[id], *target))))
                        .min_by(|a, b| {
                            a.1.total_cmp(&b.1)
                                .then_with(|| lex_order(coordinates[a.0], coordinates[b.0]))
                        });

                    match best {
                        Some((id, distance)) if distance <= 4.0 * epsilon => {
                            used[id] = true;
                            ordered_indices[vertex_index] = id;
                        }
                        _ => {
                            matched = false;
                            break;
                        }
                    }
                }

                if matched {
                    return Ok(ordered_indices);
                }
            }
        }
    }

    Err(DetectionError::InvalidCube {
        reason: "eight vertices do not form one rectangular cuboid",
    })
}

fn lex_order(a: Vector3, b: Vector3) -> Ordering {
    a[0].total_cmp(&b[0])
        .then_with(|| a[1].total_cmp(&b[1]))
        .then_with(|| a[2].total_cmp(&b[2]))
}

pub(super) fn validate_cube(points: &[[f32; 3]]) -> Result<(), DetectionError> {
    cube_indices(points).map(|_| ())
}

pub(super) fn normalize_cube(points: &[[f32; 3]]) -> Result<Vec<[f32; 3]>, DetectionError> {
    let indices = cube_indices(points)?;

    Ok(indices.iter().map(|&index| points[index]).collect())
}

#[cfg(test)]
mod tests {
    use crate::detection::{DetectionArea, DetectionAreaKind, DetectionError, DetectionStatus};

    fn area(kind: DetectionAreaKind, points: Vec<[f32; 3]>) -> DetectionArea {
        DetectionArea {
            id: 7,
            status: DetectionStatus::Off,
            kind,
            points,
        }
    }

    fn cube() -> Vec<[f32; 3]> {
        vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 2.0],
            [2.0, 0.0, 2.0],
            [2.0, 2.0, 2.0],
            [0.0, 2.0, 2.0],
        ]
    }

    #[test]
    fn polygon_normalize_only_changes_winding() {
        let points = vec![
            [0.0, 0.0, 3.0],
            [0.0, 2.0, 3.0],
            [3.0, 2.0, 3.0],
            [3.0, 0.0, 3.0],
        ];
        let mut detection_area = area(DetectionAreaKind::Polygon2D, points.clone());

        detection_area.validate().unwrap();
        detection_area.normalize().unwrap();

        assert_eq!(
            detection_area.points,
            vec![points[0], points[3], points[2], points[1]]
        );

        detection_area.validate().unwrap();

        let before = detection_area.points.clone();

        detection_area.normalize().unwrap();

        assert_eq!(detection_area.points, before);
    }

    #[test]
    fn concave_polygon_is_valid() {
        let mut detection_area = area(
            DetectionAreaKind::Polygon2D,
            vec![
                [0.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
                [3.0, 3.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 3.0, 0.0],
            ],
        );

        detection_area.validate().unwrap();
        detection_area.normalize().unwrap();

        assert_eq!(detection_area.points[0], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn tilted_polygon_is_valid() {
        let mut detection_area = area(
            DetectionAreaKind::Polygon2D,
            vec![
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 1.0],
                [2.0, 1.0, 1.0],
                [2.0, 0.0, 0.0],
            ],
        );

        detection_area.normalize().unwrap();
        detection_area.validate().unwrap();
    }

    #[test]
    fn polygon_crossing_rejected_without_mutation() {
        let mut detection_area = area(
            DetectionAreaKind::Polygon2D,
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 2.0, 0.0],
                [0.0, 2.0, 0.0],
                [2.0, 0.0, 0.0],
            ],
        );

        let snapshot = detection_area.points.clone();

        assert!(matches!(
            detection_area.validate(),
            Err(DetectionError::SelfIntersectingPolygon { .. })
        ));
        assert!(detection_area.normalize().is_err());

        assert_eq!(detection_area.points, snapshot);
    }

    #[test]
    fn polygon_nonplanar_rejected() {
        let detection_area = area(
            DetectionAreaKind::Polygon2D,
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 2.0, 0.1],
                [0.0, 2.0, 0.0],
            ],
        );

        assert_eq!(
            detection_area.validate(),
            Err(DetectionError::NonPlanarPolygon)
        );
    }

    #[test]
    fn polygon_duplicate_vertex_rejected() {
        let detection_area = area(
            DetectionAreaKind::Polygon2D,
            vec![
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0],
            ],
        );

        assert!(matches!(
            detection_area.validate(),
            Err(DetectionError::DuplicatePoint { .. })
        ));
    }

    #[test]
    fn cube_shuffled_matches_canonical_vertices() {
        let expected = cube();
        let mut detection_area = area(
            DetectionAreaKind::Cube3D,
            vec![
                expected[6],
                expected[2],
                expected[5],
                expected[0],
                expected[7],
                expected[1],
                expected[4],
                expected[3],
            ],
        );

        detection_area.validate().unwrap();
        detection_area.normalize().unwrap();

        assert_eq!(detection_area.points, expected);

        detection_area.normalize().unwrap();

        assert_eq!(detection_area.points, expected);
    }

    #[test]
    fn rotated_translated_cube_normalizes_independent_of_order() {
        // 축 회전과 이동을 적용한 정육면체의 정렬 결과를 비교한다.
        let angle = 0.47_f32;
        let (sin_yaw, cos_yaw) = angle.sin_cos();
        let transform = |point: [f32; 3]| {
            let x = cos_yaw * point[0] - sin_yaw * point[1];
            let y = sin_yaw * point[0] + cos_yaw * point[1];
            let (sin_roll, cos_roll) = 0.23_f32.sin_cos();
            [
                x + 4.0,
                cos_roll * y - sin_roll * point[2] - 2.0,
                sin_roll * y + cos_roll * point[2] + 1.0,
            ]
        };

        let points: Vec<_> = cube().into_iter().map(transform).collect();
        let mut detection_area = area(
            DetectionAreaKind::Cube3D,
            vec![
                points[5], points[0], points[3], points[2], points[4], points[7], points[1],
                points[6],
            ],
        );
        let mut other_area = area(
            DetectionAreaKind::Cube3D,
            vec![
                points[6], points[2], points[1], points[7], points[3], points[4], points[0],
                points[5],
            ],
        );

        detection_area.normalize().unwrap();
        other_area.normalize().unwrap();

        assert_eq!(detection_area.points, other_area.points);

        detection_area.validate().unwrap();
    }

    #[test]
    fn rectangular_cuboid_is_valid() {
        let mut points = cube();

        for point in &mut points {
            point[2] *= 2.0;
        }

        let detection_area = area(DetectionAreaKind::Cube3D, points);

        assert!(detection_area.validate().is_ok());
    }

    #[test]
    fn distorted_cube_is_rejected() {
        let mut points = cube();
        points[6][0] += 0.1;

        let detection_area = area(DetectionAreaKind::Cube3D, points);

        assert!(detection_area.validate().is_err());
    }

    #[test]
    fn wrong_cube_point_count_rejected() {
        let detection_area = area(DetectionAreaKind::Cube3D, cube()[..7].to_vec());

        assert!(matches!(
            detection_area.validate(),
            Err(DetectionError::InvalidPointCount { .. })
        ));
    }

    #[test]
    fn non_finite_points_rejected() {
        let mut points = cube();
        points[3][2] = f32::NAN;

        let detection_area = area(DetectionAreaKind::Cube3D, points);

        assert_eq!(
            detection_area.validate(),
            Err(DetectionError::NonFinitePoint { index: 3 })
        );
    }
}
