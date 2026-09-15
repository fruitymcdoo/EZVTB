mod animation_plugin;
mod cli;
mod rig_plugin;
mod scene_setup;

use bevy::prelude::*;
use clap::Parser;

use animation_plugin::AnimationPlugin;
use cli::Args;
use ezvtb_tracking::{FaceLandmarkTopology, FaceTracker, TrackerConfig};
use rig_plugin::{ModelPaths, RigPlugin};

fn main() {
    let args = Args::parse();

    if let Err(err) = ezvtb_tracking::init_onnx_runtime(&args.onnxruntime_dylib) {
        eprintln!("failed to load ONNX Runtime from {}: {err}", args.onnxruntime_dylib.display());
        std::process::exit(1);
    }

    let tracker = match FaceTracker::spawn(TrackerConfig {
        camera_index: args.camera_index,
        face_model_path: args.face_model.clone(),
        topology: FaceLandmarkTopology::default(),
    }) {
        Ok(tracker) => tracker,
        Err(err) => {
            eprintln!("failed to start face tracker: {err}");
            std::process::exit(1);
        }
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "EZVTB".to_string(), ..default() }),
            ..default()
        }))
        .insert_non_send_resource(tracker)
        .insert_resource(ModelPaths { model: args.model, rig_config_override: args.rig_config })
        .add_plugins((RigPlugin, AnimationPlugin))
        .add_systems(Startup, scene_setup::spawn_camera_and_lights)
        .run();
}
