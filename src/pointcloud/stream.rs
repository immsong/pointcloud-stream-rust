use std::sync::{Arc, Mutex};

use crate::pointcloud::PointCloudFrame;

/// 항상 가장 최신 Point Cloud frame만 유지하는 stream.
#[derive(Clone, Debug)]
pub struct LatestFrameStream {
    frame: Arc<Mutex<Option<PointCloudFrame>>>,
}

impl LatestFrameStream {
    pub fn new() -> Self {
        Self {
            frame: Arc::new(Mutex::new(None)),
        }
    }

    pub fn push(&self, frame: PointCloudFrame) {
        let mut latest = self.frame.lock().unwrap();
        *latest = Some(frame);
    }

    pub fn take(&self) -> Option<PointCloudFrame> {
        let mut latest = self.frame.lock().unwrap();
        latest.take()
    }
}

impl Default for LatestFrameStream {
    fn default() -> Self {
        Self::new()
    }
}
