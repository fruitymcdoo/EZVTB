use crate::bone::HumanoidBone;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// A resolved mapping from [`HumanoidBone`] slots to the actual bone/joint
/// names present in one specific rigged model.
///
/// `BTreeMap` (rather than `HashMap`) so serialized RON files come out in a
/// stable, diff-friendly order.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoneMap {
    bones: BTreeMap<HumanoidBone, String>,
}

impl BoneMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, slot: HumanoidBone) -> Option<&str> {
        self.bones.get(&slot).map(String::as_str)
    }

    pub fn set(&mut self, slot: HumanoidBone, bone_name: impl Into<String>) {
        self.bones.insert(slot, bone_name.into());
    }

    pub fn remove(&mut self, slot: HumanoidBone) -> Option<String> {
        self.bones.remove(&slot)
    }

    pub fn iter(&self) -> impl Iterator<Item = (HumanoidBone, &str)> {
        self.bones.iter().map(|(&slot, name)| (slot, name.as_str()))
    }

    /// True if every bone EZVTB's built-in face tracker needs
    /// ([`HumanoidBone::FACE_TRACKED`]) has an assignment.
    pub fn covers_face_tracking(&self) -> bool {
        HumanoidBone::FACE_TRACKED.iter().all(|b| self.bones.contains_key(b))
    }

    /// Merge `other` on top of `self`: entries present in `other` overwrite
    /// `self`'s, everything else in `self` is kept. Used to layer a
    /// hand-edited override file on top of the auto-mapped result.
    pub fn merge_from(&mut self, other: &BoneMap) {
        for (slot, name) in other.iter() {
            self.set(slot, name);
        }
    }
}

/// Per-model configuration persisted next to a model file, e.g.
/// `my_avatar.glb` -> `my_avatar.rig.ron`. Holds the bone map (auto-mapped,
/// then optionally hand-edited) plus a little metadata to sanity-check that
/// a saved config still matches the model it's loaded against.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRigConfig {
    /// File name (not full path) of the model this config was built for,
    /// stored purely as a human-readable sanity check.
    pub model_file: String,
    pub bone_map: BoneMap,
}

#[derive(Debug, thiserror::Error)]
pub enum RigConfigError {
    #[error("failed to read rig config at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse rig config at {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: Box<ron::error::SpannedError>,
    },
    #[error("failed to serialize rig config: {0}")]
    Serialize(#[from] ron::Error),
}

impl ModelRigConfig {
    pub fn new(model_file: impl Into<String>, bone_map: BoneMap) -> Self {
        Self { model_file: model_file.into(), bone_map }
    }

    /// The conventional sidecar config path for a given model path:
    /// `models/avatar.glb` -> `models/avatar.rig.ron`.
    pub fn sidecar_path(model_path: &Path) -> std::path::PathBuf {
        model_path.with_extension("rig.ron")
    }

    pub fn load(path: &Path) -> Result<Self, RigConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| RigConfigError::Io {
            path: path.display().to_string(),
            source,
        })?;
        ron::from_str(&text).map_err(|source| RigConfigError::Parse {
            path: path.display().to_string(),
            source: Box::new(source),
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), RigConfigError> {
        let pretty = ron::ser::PrettyConfig::new().struct_names(true);
        let text = ron::ser::to_string_pretty(self, pretty)?;
        std::fs::write(path, text).map_err(|source| RigConfigError::Io {
            path: path.display().to_string(),
            source,
        })
    }
}
