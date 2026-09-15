#[derive(Debug, thiserror::Error)]
pub enum TrackingError {
    #[error("camera error: {0}")]
    Camera(#[from] nokhwa::NokhwaError),

    #[error("failed to decode camera frame: {0}")]
    Decode(String),

    #[error("onnxruntime error: {0}")]
    Ort(String),

    #[error("face landmark model at {path} has no declared inputs")]
    ModelHasNoInputs { path: String },

    #[error("face landmark model at {path} has no declared outputs")]
    ModelHasNoOutputs { path: String },

    #[error("face landmark model input at {path} is not a tensor")]
    UnsupportedInputType { path: String },
}
