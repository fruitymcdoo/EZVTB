use bevy::prelude::*;

use ezvtb_rig::HumanoidBone;
use ezvtb_tracking::FaceTracker;

use crate::rig_plugin::ActiveRig;

/// How much of the tracked head rotation goes to the neck vs. the head
/// bone. Splitting it (rather than putting it all on one bone) reads as a
/// more natural head turn on most humanoid rigs.
const NECK_ROTATION_SHARE: f32 = 0.3;
const HEAD_ROTATION_SHARE: f32 = 0.7;

const MAX_JAW_OPEN_RADIANS: f32 = 0.35;

/// Time constant (seconds) for exponentially smoothing bone rotation
/// toward the latest tracked value, to soften per-frame landmark jitter
/// without adding perceptible input lag.
const SMOOTHING_TIME_CONSTANT: f32 = 0.08;

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drive_bones_from_tracking);
    }
}

fn drive_bones_from_tracking(
    tracker: NonSend<FaceTracker>,
    rig: Res<ActiveRig>,
    mut transforms: Query<&mut Transform>,
    time: Res<Time>,
) {
    let Some(update) = tracker.latest_update() else { return };
    let alpha = 1.0 - (-time.delta_secs() / SMOOTHING_TIME_CONSTANT).exp();

    if let Some(bone) = rig.get(HumanoidBone::Head) {
        let delta = Quat::IDENTITY.slerp(update.rotation, HEAD_ROTATION_SHARE);
        smooth_rotate_toward(&mut transforms, bone.entity, bone.rest_rotation * delta, alpha);
    }
    if let Some(bone) = rig.get(HumanoidBone::Neck) {
        let delta = Quat::IDENTITY.slerp(update.rotation, NECK_ROTATION_SHARE);
        smooth_rotate_toward(&mut transforms, bone.entity, bone.rest_rotation * delta, alpha);
    }
    if let Some(bone) = rig.get(HumanoidBone::Jaw) {
        let delta = Quat::from_rotation_x(update.blend.jaw_open * MAX_JAW_OPEN_RADIANS);
        smooth_rotate_toward(&mut transforms, bone.entity, bone.rest_rotation * delta, alpha);
    }
}

fn smooth_rotate_toward(transforms: &mut Query<&mut Transform>, entity: Entity, target: Quat, alpha: f32) {
    if let Ok(mut transform) = transforms.get_mut(entity) {
        transform.rotation = transform.rotation.slerp(target, alpha);
    }
}
