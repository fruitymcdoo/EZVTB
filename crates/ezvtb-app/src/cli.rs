use std::path::PathBuf;

use clap::Parser;

/// EZVTB: drive a rigged 3D model's bones from webcam face tracking.
#[derive(Debug, Parser)]
#[command(name = "ezvtb", version, about)]
pub struct Args {
    /// Path to the rigged model to load (.glb or .gltf).
    #[arg(long)]
    pub model: PathBuf,

    /// Path to an ONNX face-landmark model (e.g. a MediaPipe Face Mesh
    /// export). See the README for where to get one.
    #[arg(long)]
    pub face_model: PathBuf,

    /// Path to the ONNX Runtime shared library (onnxruntime.so / .dylib /
    /// .dll). Falls back to the ORT_DYLIB_PATH environment variable.
    #[arg(long, env = "ORT_DYLIB_PATH")]
    pub onnxruntime_dylib: PathBuf,

    /// Webcam device index.
    #[arg(long, default_value_t = 0)]
    pub camera_index: u32,

    /// Path to a hand-edited bone-map override (RON). Defaults to a
    /// `<model>.rig.ron` sidecar file next to the model.
    #[arg(long)]
    pub rig_config: Option<PathBuf>,
}
