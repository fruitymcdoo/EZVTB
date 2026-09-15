//! Webcam face tracking for EZVTB.
//!
//! Pipeline: [`camera::FaceCamera`] grabs webcam frames -> a center-square
//! crop is fed to [`landmarks::LandmarkModel`] (an ONNX face-landmark model
//! run via `ort`) -> [`pose`] turns the resulting landmarks into a head
//! rotation and simple eye/jaw blend values. [`tracker::FaceTracker`] runs
//! this loop on a background thread and publishes the latest result for
//! the render thread to poll.
//!
//! This crate has no dependency on `ezvtb-rig` or any renderer — it just
//! produces [`tracker::TrackingUpdate`]s; wiring those onto a model's bones
//! is the host application's job.

mod camera;
mod error;
mod landmarks;
mod pose;
mod tracker;

pub use camera::FaceCamera;
pub use error::TrackingError;
pub use landmarks::{Landmark3, LandmarkModel};
pub use pose::{estimate_blend_shapes, estimate_head_pose, BlendShapes, FaceLandmarkTopology, HeadPose};
pub use tracker::{FaceTracker, TrackerConfig, TrackingUpdate};

/// Point `ort` at an ONNX Runtime shared library (`onnxruntime.{so,dylib,dll}`)
/// and make it the process-wide runtime backend. Must be called once,
/// before creating any [`LandmarkModel`] / [`FaceTracker`].
pub fn init_onnx_runtime(dylib_path: &std::path::Path) -> Result<(), TrackingError> {
    ort::init_from(dylib_path.to_string_lossy().into_owned())
        .map_err(|e| TrackingError::Ort(e.to_string()))?
        .commit();
    Ok(())
}
