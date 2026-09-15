use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use glam::Quat;
use image::RgbImage;

use crate::camera::FaceCamera;
use crate::error::TrackingError;
use crate::landmarks::LandmarkModel;
use crate::pose::{self, BlendShapes, FaceLandmarkTopology};

/// The latest tracking result available to the render/app thread.
///
/// `rotation` is head orientation *relative to the calibrated neutral
/// pose* (see [`FaceTracker::recalibrate`]), so it starts at identity and
/// is safe to apply directly on top of a bone's rest rotation.
#[derive(Debug, Clone, Copy)]
pub struct TrackingUpdate {
    pub rotation: Quat,
    pub blend: BlendShapes,
    pub frame_index: u64,
}

pub struct TrackerConfig {
    pub camera_index: u32,
    pub face_model_path: PathBuf,
    pub topology: FaceLandmarkTopology,
}

enum Command {
    Recalibrate,
}

/// Runs webcam capture + landmark inference + pose estimation on a
/// dedicated background thread, and publishes the most recent result for
/// the app to poll once per render frame. Polling (rather than a blocking
/// channel) is deliberate: the app should always render with the latest
/// available tracking data and never stall a frame waiting on the camera.
pub struct FaceTracker {
    latest: Arc<Mutex<Option<TrackingUpdate>>>,
    shutdown: Arc<AtomicBool>,
    commands: crossbeam_channel::Sender<Command>,
    handle: Option<JoinHandle<()>>,
}

impl FaceTracker {
    /// Spawns the background capture/inference thread, which opens the
    /// camera and loads the landmark model itself (nokhwa's `Camera` isn't
    /// `Send`, so it can't be set up on the caller's thread and handed
    /// over). `spawn` blocks briefly until that setup finishes, so setup
    /// errors still surface synchronously to the caller instead of being
    /// discovered later.
    pub fn spawn(config: TrackerConfig) -> Result<Self, TrackingError> {
        let latest = Arc::new(Mutex::new(None));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let (init_tx, init_rx) = crossbeam_channel::bounded(1);

        let latest_thread = latest.clone();
        let shutdown_thread = shutdown.clone();
        let topology = config.topology;

        let handle = std::thread::Builder::new()
            .name("ezvtb-face-tracker".into())
            .spawn(move || {
                let mut camera = match FaceCamera::open(config.camera_index) {
                    Ok(camera) => camera,
                    Err(err) => {
                        let _ = init_tx.send(Err(err));
                        return;
                    }
                };
                let mut model = match LandmarkModel::load(&config.face_model_path) {
                    Ok(model) => model,
                    Err(err) => {
                        let _ = init_tx.send(Err(err));
                        return;
                    }
                };
                let _ = init_tx.send(Ok(()));

                let mut calibration: Option<Quat> = None;
                let mut frame_index = 0u64;

                while !shutdown_thread.load(Ordering::Relaxed) {
                    while let Ok(cmd) = command_rx.try_recv() {
                        match cmd {
                            Command::Recalibrate => calibration = None,
                        }
                    }

                    let frame = match camera.grab_frame() {
                        Ok(frame) => frame,
                        Err(err) => {
                            tracing::warn!("camera read failed: {err}");
                            std::thread::sleep(Duration::from_millis(200));
                            continue;
                        }
                    };
                    let crop = center_square_crop(&frame);

                    let landmarks = match model.infer(&crop) {
                        Ok(landmarks) => landmarks,
                        Err(err) => {
                            tracing::warn!("landmark inference failed: {err}");
                            continue;
                        }
                    };

                    let Some(head_pose) = pose::estimate_head_pose(&landmarks, &topology) else {
                        continue;
                    };
                    let blend = pose::estimate_blend_shapes(&landmarks, &topology).unwrap_or_default();

                    let neutral = *calibration.get_or_insert(head_pose.rotation);
                    let relative_rotation = neutral.inverse() * head_pose.rotation;

                    frame_index += 1;
                    *latest_thread.lock().unwrap() =
                        Some(TrackingUpdate { rotation: relative_rotation, blend, frame_index });
                }
            })
            .expect("failed to spawn face tracker thread");

        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self { latest, shutdown, commands: command_tx, handle: Some(handle) }),
            Ok(Err(err)) => {
                let _ = handle.join();
                Err(err)
            }
            Err(_) => {
                let _ = handle.join();
                Err(TrackingError::Decode("face tracker thread exited before initializing".into()))
            }
        }
    }

    /// The most recent tracking result, if the background thread has
    /// produced one yet.
    pub fn latest_update(&self) -> Option<TrackingUpdate> {
        *self.latest.lock().unwrap()
    }

    /// Re-center: treat whatever head pose the next frame reports as the
    /// new neutral (zero-rotation) pose. Call this when the user faces the
    /// camera normally, e.g. bound to a "center face" hotkey.
    pub fn recalibrate(&self) {
        let _ = self.commands.send(Command::Recalibrate);
    }
}

impl Drop for FaceTracker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Crop the largest centered square out of a frame. This assumes the
/// subject's face is roughly centered in the webcam frame (a reasonable
/// default for a typical desk-mounted vtuber camera setup). A dedicated
/// face-detector pass to locate and track the face box is the natural next
/// step if that assumption doesn't hold — see the project roadmap.
fn center_square_crop(img: &RgbImage) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let x = (w - side) / 2;
    let y = (h - side) / 2;
    image::imageops::crop_imm(img, x, y, side, side).to_image()
}
