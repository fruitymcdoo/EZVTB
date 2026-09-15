use serde::{Deserialize, Serialize};
use std::fmt;

/// A canonical humanoid bone slot, modeled after the VRM "Humanoid" bone set
/// (itself modeled after Unity's Humanoid avatar system). This is the
/// vocabulary EZVTB uses internally; a [`crate::BoneMap`] links each of
/// these to the actual bone name found in a specific rigged model.
///
/// The set covers the full body so the same rig can later be driven by
/// hand/body trackers, not just the face tracker EZVTB ships with today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HumanoidBone {
    Hips,
    Spine,
    Chest,
    UpperChest,
    Neck,
    Head,
    LeftEye,
    RightEye,
    Jaw,

    LeftShoulder,
    LeftUpperArm,
    LeftLowerArm,
    LeftHand,
    RightShoulder,
    RightUpperArm,
    RightLowerArm,
    RightHand,

    LeftUpperLeg,
    LeftLowerLeg,
    LeftFoot,
    LeftToes,
    RightUpperLeg,
    RightLowerLeg,
    RightFoot,
    RightToes,
}

impl HumanoidBone {
    /// All bone slots, in a stable, human-friendly order.
    pub const ALL: &'static [HumanoidBone] = &[
        HumanoidBone::Hips,
        HumanoidBone::Spine,
        HumanoidBone::Chest,
        HumanoidBone::UpperChest,
        HumanoidBone::Neck,
        HumanoidBone::Head,
        HumanoidBone::LeftEye,
        HumanoidBone::RightEye,
        HumanoidBone::Jaw,
        HumanoidBone::LeftShoulder,
        HumanoidBone::LeftUpperArm,
        HumanoidBone::LeftLowerArm,
        HumanoidBone::LeftHand,
        HumanoidBone::RightShoulder,
        HumanoidBone::RightUpperArm,
        HumanoidBone::RightLowerArm,
        HumanoidBone::RightHand,
        HumanoidBone::LeftUpperLeg,
        HumanoidBone::LeftLowerLeg,
        HumanoidBone::LeftFoot,
        HumanoidBone::LeftToes,
        HumanoidBone::RightUpperLeg,
        HumanoidBone::RightLowerLeg,
        HumanoidBone::RightFoot,
        HumanoidBone::RightToes,
    ];

    /// Bones EZVTB's built-in webcam face tracker can drive today.
    /// Everything else in [`HumanoidBone::ALL`] is still auto-mapped and
    /// saved to the rig config so future trackers (hands, full body) have
    /// somewhere to plug in without redoing bone assignment.
    pub const FACE_TRACKED: &'static [HumanoidBone] = &[
        HumanoidBone::Neck,
        HumanoidBone::Head,
        HumanoidBone::LeftEye,
        HumanoidBone::RightEye,
        HumanoidBone::Jaw,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HumanoidBone::Hips => "Hips",
            HumanoidBone::Spine => "Spine",
            HumanoidBone::Chest => "Chest",
            HumanoidBone::UpperChest => "UpperChest",
            HumanoidBone::Neck => "Neck",
            HumanoidBone::Head => "Head",
            HumanoidBone::LeftEye => "LeftEye",
            HumanoidBone::RightEye => "RightEye",
            HumanoidBone::Jaw => "Jaw",
            HumanoidBone::LeftShoulder => "LeftShoulder",
            HumanoidBone::LeftUpperArm => "LeftUpperArm",
            HumanoidBone::LeftLowerArm => "LeftLowerArm",
            HumanoidBone::LeftHand => "LeftHand",
            HumanoidBone::RightShoulder => "RightShoulder",
            HumanoidBone::RightUpperArm => "RightUpperArm",
            HumanoidBone::RightLowerArm => "RightLowerArm",
            HumanoidBone::RightHand => "RightHand",
            HumanoidBone::LeftUpperLeg => "LeftUpperLeg",
            HumanoidBone::LeftLowerLeg => "LeftLowerLeg",
            HumanoidBone::LeftFoot => "LeftFoot",
            HumanoidBone::LeftToes => "LeftToes",
            HumanoidBone::RightUpperLeg => "RightUpperLeg",
            HumanoidBone::RightLowerLeg => "RightLowerLeg",
            HumanoidBone::RightFoot => "RightFoot",
            HumanoidBone::RightToes => "RightToes",
        }
    }
}

impl fmt::Display for HumanoidBone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
