use glam::{Mat3, Quat, Vec3};

use crate::landmarks::Landmark3;

/// Indices into a model's landmark output that this crate needs to know
/// about to compute head pose and simple facial blend values. The defaults
/// ([`FaceLandmarkTopology::mediapipe_face_mesh_468`]) match the canonical
/// 468-point MediaPipe Face Mesh topology, which is what most publicly
/// available face-landmark ONNX exports (and their derivatives) use. If
/// your model uses a different point set, build a custom `FaceLandmarkTopology`
/// with the matching indices.
#[derive(Debug, Clone, Copy)]
pub struct FaceLandmarkTopology {
    pub left_eye_outer: usize,
    pub left_eye_inner: usize,
    pub left_eye_top: usize,
    pub left_eye_bottom: usize,
    pub right_eye_outer: usize,
    pub right_eye_inner: usize,
    pub right_eye_top: usize,
    pub right_eye_bottom: usize,
    pub nose_tip: usize,
    pub chin: usize,
    pub mouth_left: usize,
    pub mouth_right: usize,
    pub mouth_top: usize,
    pub mouth_bottom: usize,
}

impl FaceLandmarkTopology {
    /// Indices for the standard 468-point MediaPipe Face Mesh topology.
    /// "Left"/"right" follow MediaPipe's convention, which is the subject's
    /// own left/right (mirrored relative to a selfie-view image).
    pub const fn mediapipe_face_mesh_468() -> Self {
        Self {
            left_eye_outer: 33,
            left_eye_inner: 133,
            left_eye_top: 159,
            left_eye_bottom: 145,
            right_eye_outer: 263,
            right_eye_inner: 362,
            right_eye_top: 386,
            right_eye_bottom: 374,
            nose_tip: 1,
            chin: 152,
            mouth_left: 61,
            mouth_right: 291,
            mouth_top: 13,
            mouth_bottom: 14,
        }
    }

    fn max_index(&self) -> usize {
        [
            self.left_eye_outer,
            self.left_eye_inner,
            self.left_eye_top,
            self.left_eye_bottom,
            self.right_eye_outer,
            self.right_eye_inner,
            self.right_eye_top,
            self.right_eye_bottom,
            self.nose_tip,
            self.chin,
            self.mouth_left,
            self.mouth_right,
            self.mouth_top,
            self.mouth_bottom,
        ]
        .into_iter()
        .max()
        .unwrap_or(0)
    }
}

impl Default for FaceLandmarkTopology {
    fn default() -> Self {
        Self::mediapipe_face_mesh_468()
    }
}

/// Head orientation derived from a single frame's landmarks, as a rotation
/// in camera space (x right, y down, z toward the camera). This is a raw
/// per-frame estimate; [`crate::tracker::FaceTracker`] applies calibration
/// on top of it so a neutral head pose reads as "no rotation" regardless of
/// webcam mounting angle.
#[derive(Debug, Clone, Copy)]
pub struct HeadPose {
    pub rotation: Quat,
}

/// Simple ARKit/VRM-style facial blend values in `[0, 1]`, where `0` is
/// neutral (eyes open, mouth closed) and `1` is fully activated (eyes
/// closed, mouth fully open).
#[derive(Debug, Clone, Copy, Default)]
pub struct BlendShapes {
    pub eye_blink_left: f32,
    pub eye_blink_right: f32,
    pub jaw_open: f32,
}

fn vec3(p: Landmark3) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

fn dist2(a: Landmark3, b: Landmark3) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Estimate head rotation from three roughly-rigid facial landmarks (eye
/// corners and nose bridge). This is a lightweight geometric approximation,
/// not a full `solvePnP` fit against a 3D face model — it's cheap, dependency-free,
/// and good enough to drive a head/neck bone, but it will be less accurate
/// at extreme angles than a proper PnP solve (see the project roadmap).
pub fn estimate_head_pose(landmarks: &[Landmark3], topology: &FaceLandmarkTopology) -> Option<HeadPose> {
    if landmarks.len() <= topology.max_index() {
        return None;
    }

    let left_eye = vec3(landmarks[topology.left_eye_outer]).midpoint(vec3(landmarks[topology.left_eye_inner]));
    let right_eye = vec3(landmarks[topology.right_eye_outer]).midpoint(vec3(landmarks[topology.right_eye_inner]));
    let nose = vec3(landmarks[topology.nose_tip]);
    let chin = vec3(landmarks[topology.chin]);

    // Build an orthonormal basis for the face: `right` runs across the
    // eyes, `down` runs from the eye line toward the chin, and `forward`
    // is whatever's left to make the three mutually perpendicular. This
    // directly gives us a rotation without decomposing to Euler angles
    // first (which would risk gimbal lock at extreme poses).
    let right = (right_eye - left_eye).try_normalize()?;
    let down_ref = (chin - nose).try_normalize()?;
    let forward = right.cross(down_ref).try_normalize()?;
    let down = forward.cross(right).try_normalize()?;

    let basis = Mat3::from_cols(right, down, forward);
    Some(HeadPose { rotation: Quat::from_mat3(&basis) })
}

/// Eye-aspect-ratio-style openness for one eye: vertical eyelid distance
/// over horizontal eye-corner distance. Falls as the eye closes.
fn eye_openness(landmarks: &[Landmark3], outer: usize, inner: usize, top: usize, bottom: usize) -> Option<f32> {
    let horiz = dist2(landmarks[outer], landmarks[inner]);
    if horiz <= f32::EPSILON {
        return None;
    }
    Some(dist2(landmarks[top], landmarks[bottom]) / horiz)
}

/// Convert eyes/mouth landmark geometry into blend values.
///
/// The openness -> blend thresholds below are reasonable defaults for the
/// MediaPipe Face Mesh topology, but eye and mouth proportions vary by
/// face and camera angle; treat these as a starting calibration, not a
/// universal constant.
pub fn estimate_blend_shapes(landmarks: &[Landmark3], topology: &FaceLandmarkTopology) -> Option<BlendShapes> {
    if landmarks.len() <= topology.max_index() {
        return None;
    }

    const EAR_CLOSED: f32 = 0.10;
    const EAR_OPEN: f32 = 0.30;
    let blink_from_ear = |ear: f32| 1.0 - ((ear - EAR_CLOSED) / (EAR_OPEN - EAR_CLOSED)).clamp(0.0, 1.0);

    let left_ear = eye_openness(
        landmarks,
        topology.left_eye_outer,
        topology.left_eye_inner,
        topology.left_eye_top,
        topology.left_eye_bottom,
    )?;
    let right_ear = eye_openness(
        landmarks,
        topology.right_eye_outer,
        topology.right_eye_inner,
        topology.right_eye_top,
        topology.right_eye_bottom,
    )?;

    let mouth_width = dist2(landmarks[topology.mouth_left], landmarks[topology.mouth_right]);
    let jaw_open = if mouth_width > f32::EPSILON {
        const MOUTH_OPEN_RATIO_MAX: f32 = 0.6;
        (dist2(landmarks[topology.mouth_top], landmarks[topology.mouth_bottom]) / mouth_width / MOUTH_OPEN_RATIO_MAX)
            .clamp(0.0, 1.0)
    } else {
        0.0
    };

    Some(BlendShapes {
        eye_blink_left: blink_from_ear(left_ear),
        eye_blink_right: blink_from_ear(right_ear),
        jaw_open,
    })
}
