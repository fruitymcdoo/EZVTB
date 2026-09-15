//! Model-agnostic humanoid rig abstraction for EZVTB.
//!
//! This crate knows nothing about rendering, ONNX, or webcams. It defines:
//!
//! - [`HumanoidBone`]: a canonical vocabulary of bone slots (head, neck,
//!   eyes, jaw, arms, legs, ...), modeled after the VRM/Unity Humanoid bone
//!   set.
//! - [`automap::auto_map_bones`]: a heuristic that guesses a [`BoneMap`]
//!   from the raw joint names of an arbitrary loaded rig (Mixamo, VRM/Unity
//!   Humanoid, and similar conventions).
//! - [`BoneMap`] / [`ModelRigConfig`]: the resolved mapping, and a RON
//!   sidecar file format so a person can hand-correct the guess once and
//!   have it stick.

mod automap;
mod bone;
mod config;

pub use automap::auto_map_bones;
pub use bone::HumanoidBone;
pub use config::{BoneMap, ModelRigConfig, RigConfigError};
