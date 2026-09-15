use image::RgbImage;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;

use crate::error::TrackingError;

/// A thin wrapper around a [`nokhwa::Camera`] that always hands back a
/// decoded [`RgbImage`].
pub struct FaceCamera {
    camera: Camera,
}

impl FaceCamera {
    pub fn open(camera_index: u32) -> Result<Self, TrackingError> {
        let index = CameraIndex::Index(camera_index);
        let format = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
        let mut camera = Camera::new(index, format)?;
        camera.open_stream()?;
        Ok(Self { camera })
    }

    pub fn grab_frame(&mut self) -> Result<RgbImage, TrackingError> {
        let buffer = self.camera.frame()?;
        buffer
            .decode_image::<RgbFormat>()
            .map_err(|e| TrackingError::Decode(e.to_string()))
    }
}
