use std::collections::HashMap;
use std::path::PathBuf;

use bevy::gltf::GltfAssetLabel;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

use ezvtb_rig::{auto_map_bones, BoneMap, HumanoidBone, ModelRigConfig};

/// Where to find the model and (optionally) a hand-edited bone-map
/// override, set once at startup from CLI args.
#[derive(Resource, Clone)]
pub struct ModelPaths {
    pub model: PathBuf,
    pub rig_config_override: Option<PathBuf>,
}

/// Marks the entity that owns the avatar's [`SceneRoot`].
#[derive(Component)]
struct AvatarRoot;

/// One resolved humanoid bone: which entity it is, and the local rotation
/// it had at bind time (so tracking can apply rotation *on top of* the
/// rig's own rest pose instead of overwriting it).
#[derive(Debug, Clone, Copy)]
pub struct RigBone {
    pub entity: Entity,
    pub rest_rotation: Quat,
}

/// The bone map resolved for the currently loaded model, filled in once
/// the model's scene has finished spawning. Absent until then.
#[derive(Resource, Default)]
pub struct ActiveRig {
    pub bones: HashMap<HumanoidBone, RigBone>,
}

impl ActiveRig {
    pub fn get(&self, slot: HumanoidBone) -> Option<&RigBone> {
        self.bones.get(&slot)
    }
}

pub struct RigPlugin;

impl Plugin for RigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveRig>().add_systems(Startup, spawn_avatar);
    }
}

fn spawn_avatar(mut commands: Commands, asset_server: Res<AssetServer>, model_paths: Res<ModelPaths>) {
    let scene: Handle<Scene> = asset_server.load(GltfAssetLabel::Scene(0).from_asset(model_paths.model.clone()));
    commands
        .spawn((SceneRoot(scene), Transform::default(), Visibility::default(), AvatarRoot))
        .observe(on_avatar_scene_ready);
}

fn on_avatar_scene_ready(
    trigger: On<SceneInstanceReady>,
    mut commands: Commands,
    model_paths: Res<ModelPaths>,
    children_q: Query<&Children>,
    skinned_q: Query<&SkinnedMesh>,
    name_q: Query<&Name>,
    transform_q: Query<&Transform>,
) {
    let root = trigger.entity;

    let mut descendants = Vec::new();
    collect_descendants(root, &children_q, &mut descendants);

    let skinned_mesh = descendants
        .iter()
        .filter_map(|&e| skinned_q.get(e).ok())
        .max_by_key(|skinned| skinned.joints.len());

    let Some(skinned_mesh) = skinned_mesh else {
        warn!(
            "model {} has no skinned mesh (no bones to drive) - it will render as a static prop",
            model_paths.model.display()
        );
        return;
    };

    let mut name_to_entity: HashMap<String, Entity> = HashMap::new();
    let mut joint_names: Vec<String> = Vec::with_capacity(skinned_mesh.joints.len());
    for &joint in &skinned_mesh.joints {
        let name = name_q.get(joint).map(|n| n.as_str().to_string()).unwrap_or_default();
        if !name.is_empty() {
            name_to_entity.insert(name.clone(), joint);
        }
        joint_names.push(name);
    }

    let mut bone_map = auto_map_bones(&joint_names);

    let sidecar_path = model_paths
        .rig_config_override
        .clone()
        .unwrap_or_else(|| ModelRigConfig::sidecar_path(&model_paths.model));

    match ModelRigConfig::load(&sidecar_path) {
        Ok(saved) => {
            info!("loaded bone-map override from {}", sidecar_path.display());
            bone_map.merge_from(&saved.bone_map);
        }
        Err(_) => {
            let model_file = model_paths
                .model
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let config = ModelRigConfig::new(model_file, bone_map.clone());
            if let Err(err) = config.save(&sidecar_path) {
                warn!("could not save auto-mapped bone config to {}: {err}", sidecar_path.display());
            } else {
                info!(
                    "auto-mapped {} bones and saved them to {} - edit that file to correct any of them",
                    bone_map_len(&bone_map),
                    sidecar_path.display()
                );
            }
        }
    }

    let mut bones = HashMap::new();
    for slot in HumanoidBone::ALL.iter().copied() {
        let Some(bone_name) = bone_map.get(slot) else { continue };
        let Some(&entity) = name_to_entity.get(bone_name) else {
            warn!("bone map assigns {slot} to '{bone_name}', but no such bone was found in the model");
            continue;
        };
        let rest_rotation = transform_q.get(entity).map(|t| t.rotation).unwrap_or(Quat::IDENTITY);
        bones.insert(slot, RigBone { entity, rest_rotation });
    }

    if !bone_map.covers_face_tracking() {
        warn!("bone map is missing one or more bones EZVTB's face tracker needs (Head/Neck/Jaw/Eyes) - webcam tracking will only move the bones that were found");
    }

    commands.insert_resource(ActiveRig { bones });
}

fn bone_map_len(map: &BoneMap) -> usize {
    map.iter().count()
}

fn collect_descendants(entity: Entity, children_q: &Query<&Children>, out: &mut Vec<Entity>) {
    out.push(entity);
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            collect_descendants(child, children_q, out);
        }
    }
}
