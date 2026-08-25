use pointcloud_stream::pointcloud::{Endianness, LatestFrameStream, PointCloudFrame};

#[test]
fn stream_take_returns_latest_frame() {
    let stream = LatestFrameStream::new();

    stream.push(PointCloudFrame {
        timestamp_ns: 1,
        frame_id: "lidar".to_string(),
        width: 1,
        height: 1,
        fields: vec![],
        endianness: Endianness::Little,
        point_step: 0,
        row_step: 0,
        is_dense: true,
        data: vec![],
    });

    stream.push(PointCloudFrame {
        timestamp_ns: 2,
        frame_id: "lidar".to_string(),
        width: 1,
        height: 1,
        fields: vec![],
        endianness: Endianness::Little,
        point_step: 0,
        row_step: 0,
        is_dense: true,
        data: vec![],
    });

    let frame = stream.take().unwrap();

    assert_eq!(frame.timestamp_ns, 2);
    assert!(stream.take().is_none());
}

#[test]
fn stream_shares_latest_frame_between_threads() {
    let stream = LatestFrameStream::new();
    let producer = stream.clone();

    let handle = std::thread::spawn(move || {
        producer.push(PointCloudFrame {
            timestamp_ns: 10,
            frame_id: "lidar".to_string(),
            width: 1,
            height: 1,
            fields: vec![],
            endianness: Endianness::Little,
            point_step: 0,
            row_step: 0,
            is_dense: true,
            data: vec![],
        });

        producer.push(PointCloudFrame {
            timestamp_ns: 20,
            frame_id: "lidar".to_string(),
            width: 1,
            height: 1,
            fields: vec![],
            endianness: Endianness::Little,
            point_step: 0,
            row_step: 0,
            is_dense: true,
            data: vec![],
        });
    });

    handle.join().unwrap();

    let frame = stream.take().unwrap();

    assert_eq!(frame.timestamp_ns, 20);
}
