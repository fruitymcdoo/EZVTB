use std::path::Path;

use image::RgbImage;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::{Shape, Tensor, ValueType};

use crate::error::TrackingError;

/// One 3D facial landmark, in normalized `[0, 1]` coordinates relative to
/// the face crop that was fed to the model (`z` is relative depth, in the
/// same units as `x`, and has no fixed sign convention across models).
#[derive(Debug, Clone, Copy, Default)]
pub struct Landmark3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy)]
enum Layout {
    /// `[N, C, H, W]`
    Nchw,
    /// `[N, H, W, C]`
    Nhwc,
}

/// Wraps an ONNX face-landmark model (e.g. a MediaPipe Face Mesh export) run
/// through `ort`. The model's expected input resolution and channel layout
/// are read from the model itself, so this works with any single-input,
/// single-output landmark model that takes a square-ish RGB face crop and
/// emits a flat array of 2D or 3D points.
pub struct LandmarkModel {
    session: Session,
    input_name: String,
    output_name: String,
    input_width: u32,
    input_height: u32,
    layout: Layout,
}

impl LandmarkModel {
    pub fn load(model_path: &Path) -> Result<Self, TrackingError> {
        let path_str = model_path.display().to_string();

        let builder = Session::builder()
            .map_err(|e| TrackingError::Ort(e.to_string()))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| TrackingError::Ort(e.to_string()))?
            .with_intra_threads(1)
            .map_err(|e| TrackingError::Ort(e.to_string()))?;
        let mut builder = builder;
        let session = builder
            .commit_from_file(model_path)
            .map_err(|e| TrackingError::Ort(e.to_string()))?;

        let input = session
            .inputs()
            .first()
            .ok_or_else(|| TrackingError::ModelHasNoInputs { path: path_str.clone() })?;
        let input_name = input.name().to_string();
        let (input_width, input_height, layout) = match input.dtype() {
            ValueType::Tensor { shape, .. } => infer_layout(shape),
            _ => return Err(TrackingError::UnsupportedInputType { path: path_str.clone() }),
        };

        let output = session
            .outputs()
            .first()
            .ok_or(TrackingError::ModelHasNoOutputs { path: path_str })?;
        let output_name = output.name().to_string();

        Ok(Self { session, input_name, output_name, input_width, input_height, layout })
    }

    pub fn input_size(&self) -> (u32, u32) {
        (self.input_width, self.input_height)
    }

    pub fn infer(&mut self, face_crop: &RgbImage) -> Result<Vec<Landmark3>, TrackingError> {
        let needs_resize = face_crop.width() != self.input_width || face_crop.height() != self.input_height;
        let resized;
        let img = if needs_resize {
            resized = image::imageops::resize(
                face_crop,
                self.input_width,
                self.input_height,
                image::imageops::FilterType::Triangle,
            );
            &resized
        } else {
            face_crop
        };

        let data = self.pack_input(img);
        let shape: Vec<i64> = match self.layout {
            Layout::Nchw => vec![1, 3, self.input_height as i64, self.input_width as i64],
            Layout::Nhwc => vec![1, self.input_height as i64, self.input_width as i64, 3],
        };
        let tensor = Tensor::from_array((shape, data)).map_err(|e| TrackingError::Ort(e.to_string()))?;

        let outputs = self
            .session
            .run(ort::inputs![self.input_name.as_str() => tensor])
            .map_err(|e| TrackingError::Ort(e.to_string()))?;
        let output_value = &outputs[self.output_name.as_str()];
        let (_, raw) = output_value
            .try_extract_tensor::<f32>()
            .map_err(|e| TrackingError::Ort(e.to_string()))?;

        Ok(unpack_landmarks(raw, self.input_width, self.input_height))
    }

    fn pack_input(&self, img: &RgbImage) -> Vec<f32> {
        let (w, h) = (self.input_width as usize, self.input_height as usize);
        let mut data = vec![0f32; w * h * 3];
        match self.layout {
            Layout::Nchw => {
                for (x, y, px) in img.enumerate_pixels() {
                    let (x, y) = (x as usize, y as usize);
                    for c in 0..3 {
                        data[c * w * h + y * w + x] = px.0[c] as f32 / 255.0;
                    }
                }
            }
            Layout::Nhwc => {
                for (x, y, px) in img.enumerate_pixels() {
                    let (x, y) = (x as usize, y as usize);
                    for c in 0..3 {
                        data[(y * w + x) * 3 + c] = px.0[c] as f32 / 255.0;
                    }
                }
            }
        }
        data
    }
}

/// Guess the model's expected `(width, height, layout)` from its declared
/// input shape. Dynamic dimensions (`-1`) fall back to 192px, a common
/// export size for lightweight face-landmark models.
fn infer_layout(shape: &Shape) -> (u32, u32, Layout) {
    const FALLBACK: u32 = 192;
    let dim_or = |d: i64| if d > 0 { d as u32 } else { FALLBACK };

    if shape.len() == 4 && shape[1] == 3 {
        (dim_or(shape[3]), dim_or(shape[2]), Layout::Nchw)
    } else if shape.len() == 4 && shape[3] == 3 {
        (dim_or(shape[2]), dim_or(shape[1]), Layout::Nhwc)
    } else {
        // Unrecognized rank/shape: assume NHWC square input as a best guess.
        (FALLBACK, FALLBACK, Layout::Nhwc)
    }
}

/// Parse the model's flat output into landmarks, tolerating both `(x, y, z)`
/// and `(x, y)` layouts, and both pixel-space and pre-normalized `[0, 1]`
/// coordinate conventions (different public landmark model exports use
/// different conventions here, so this picks based on the data's own
/// magnitude rather than assuming one).
fn unpack_landmarks(data: &[f32], input_w: u32, input_h: u32) -> Vec<Landmark3> {
    let per_point = if !data.is_empty() && data.len().is_multiple_of(3) { 3 } else { 2 };
    let points: Vec<[f32; 3]> = data
        .chunks_exact(per_point)
        .map(|c| [c[0], c[1], if per_point == 3 { c[2] } else { 0.0 }])
        .collect();

    let max_coord = points.iter().flat_map(|p| [p[0].abs(), p[1].abs()]).fold(0.0f32, f32::max);
    let (scale_x, scale_y) = if max_coord > 2.0 {
        (input_w as f32, input_h as f32)
    } else {
        (1.0, 1.0)
    };

    points
        .into_iter()
        .map(|p| Landmark3 { x: p[0] / scale_x, y: p[1] / scale_y, z: p[2] / scale_x })
        .collect()
}
