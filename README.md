# EZVTB

A from-scratch, permissively-licensed VTuber-style face tracker -> rigged model
mover, written in Rust.

Point it at your webcam and a rigged 3D model; it estimates your head pose and
a couple of facial blend values, automatically figures out which bones in the
model correspond to which body parts, and drives them live.

## Why this exists / design goals

- **No copyleft dependencies.** Every crate this project depends on is
  MIT, Apache-2.0, BSD, or similarly permissive. EZVTB itself is dual-licensed
  `MIT OR Apache-2.0` (the standard Rust ecosystem convention), so you keep
  full control over how you license and distribute your own build.
- **Pure Rust where it matters for speed.** Face landmark inference runs
  in-process through [`ort`](https://ort.pyke.io) (Rust bindings to ONNX
  Runtime) - no embedded Python, no IPC to a separate process.
- **Works with (almost) any rigged model.** Bones aren't hardcoded to one
  model. EZVTB reads whatever skeleton your model has and guesses a mapping
  automatically; you correct anything it gets wrong in a plain text file.

## How it's built

Three crates, each independently useful:

| Crate | Depends on | What it does |
|---|---|---|
| [`ezvtb-rig`](crates/ezvtb-rig) | nothing but `serde`/`ron` | Defines a canonical `HumanoidBone` vocabulary (Hips, Spine, Neck, Head, arms, legs, ...) and a heuristic `auto_map_bones()` that guesses which bone in *your* model's skeleton is which, by pattern-matching common naming conventions (Mixamo, VRM/Unity Humanoid, and similar). Also defines the `.rig.ron` sidecar file format used to save/load hand-corrected mappings. |
| [`ezvtb-tracking`](crates/ezvtb-tracking) | `nokhwa`, `ort`, `image` | Grabs webcam frames, runs an ONNX face-landmark model, and turns the landmarks into a head-rotation quaternion (via a lightweight 3-point orthonormal-frame estimate, not a full solvePnP) plus simple eye-blink/jaw-open blend values. Runs on a background thread; the app polls the latest result once per render frame. |
| [`ezvtb-app`](crates/ezvtb-app) | `bevy`, the two crates above | The actual desktop application: loads a glTF/GLB model, spawns it, walks its skeleton through `ezvtb-rig`'s auto-mapper, and each frame applies the tracker's output to the mapped bones. |

Nothing here is Bevy-specific except `ezvtb-app` - `ezvtb-rig` and
`ezvtb-tracking` are plain Rust libraries you could drop into a different
renderer if you wanted to.

### How automatic bone assignment works

On first load, EZVTB reads every joint name in your model's skeleton, tokenizes
them (handling `camelCase`, `snake_case`, `mixamorig:Prefixed`, `.L`/`.R`
suffixes, etc.), and scores each one against known naming patterns for VRM/Unity
Humanoid rigs, Mixamo rigs, and close relatives (Ready Player Me and similar).
The result is saved next to your model as `<model>.rig.ron` - a plain text
file you can open and edit by hand to fix anything it guessed wrong, or to
fill in bones it couldn't identify. On the next run, your edits are loaded
and override the auto-mapped guess for that bone.

This is a heuristic, not magic: unusual bone-naming schemes will need more
manual correction than Mixamo/VRM-derived rigs. That's by design - guess what's
obvious, make the rest a quick edit instead of a from-scratch setup.

### What actually moves right now

The built-in webcam tracker drives: **Head**, **Neck** (split so a "head turn"
looks like a head turn and not just a floating swivel), and **Jaw** (mouth
open amount, if your rig has a jaw bone). `ezvtb-rig`'s bone vocabulary
also covers the rest of a humanoid skeleton (spine, arms, legs, ...) - those
get auto-mapped and saved too, ready for a future body/hand tracker to drive
without redoing bone assignment.

Eye-blink and gaze are estimated by the tracker (see `BlendShapes` /
`HeadPose` in `ezvtb-tracking`) but not wired to anything yet, since most
generic rigs don't have eyelid *bones* (VRM models usually blink via morph
targets instead) - see Roadmap.

## Getting it running

### 1. Build

```sh
cargo build --release
```

On Linux you'll need the usual Bevy system dependencies - X11/Wayland,
ALSA, and udev development headers (e.g. on Debian/Ubuntu:
`apt install libwayland-dev libxkbcommon-dev libx11-dev libxi-dev
libxcursor-dev libxrandr-dev libudev-dev libasound2-dev`). If `cargo build`
fails on a missing system library, search "Bevy Linux dependencies" for your
distro's package names. macOS and Windows need no extra setup beyond a
working Rust toolchain.

### 2. Get an ONNX Runtime shared library

EZVTB loads ONNX Runtime dynamically at startup rather than statically linking
or auto-downloading it at build time, so you point it at a copy you trust.
Grab a release for your platform from the
[Microsoft ONNX Runtime releases page](https://github.com/microsoft/onnxruntime/releases)
(MIT-licensed) and note the path to `onnxruntime.dll` / `libonnxruntime.so` /
`libonnxruntime.dylib` inside it.

### 3. Get a face-landmark ONNX model

EZVTB doesn't ship a model file (it's a large binary asset, and you should
know what you're running). It works with any single-input, single-output
ONNX model that takes a roughly-square RGB face crop and outputs a flat array
of 2D or 3D facial landmark points - `ezvtb-tracking` reads the expected input
size/layout from the model itself, and auto-detects whether the output is
pixel-space or pre-normalized coordinates.

The 468-point **MediaPipe Face Mesh** topology (Apache-2.0, from Google) is
what the default landmark-index constants in `ezvtb-tracking::pose` assume;
search for an ONNX export of it (several exist in the open-source model-zoo
community) or convert one yourself with `tf2onnx`/`onnx-tf` from the original
TensorFlow Lite model. If you use a model with a different point ordering,
build a custom `FaceLandmarkTopology` (see `ezvtb-tracking/src/pose.rs`)
with the matching indices.

### 4. Run it

```sh
cargo run --release -- \
  --model path/to/your_avatar.glb \
  --face-model path/to/face_landmark.onnx \
  --onnxruntime-dylib path/to/libonnxruntime.so \
  --camera-index 0
```

(`--onnxruntime-dylib` can also be set via the `ORT_DYLIB_PATH` environment
variable.) On first run, look for `your_avatar.rig.ron` next to the model -
that's the auto-mapped bone assignment; edit it and re-run to correct anything.

## Current limitations / roadmap

This is a from-the-ground-up MVP, built to be a solid foundation rather than
a feature-complete VTuber suite on day one. Known gaps, roughly in the order
they're worth tackling next:

- **glTF/GLB only for now.** Bevy's built-in asset loaders don't cover
  FBX or OBJ, and FBX in particular has no mature permissively-licensed pure-Rust
  parser. The realistic path there is binding to
  [Assimp](https://github.com/assimp/assimp) (BSD-3-Clause) for import and
  converting into Bevy's mesh/skeleton types - a meaningfully separate chunk of
  work from everything else here, deliberately left for a follow-up rather than
  half-done now.
- **Face detection is a naive center-crop**, not a real detector. It assumes
  your face is roughly centered in the webcam frame (fine for a typical
  desk-mounted setup, fragile otherwise). Adding a lightweight face-detector
  ONNX pass (e.g. a BlazeFace/SCRFD export) ahead of the landmark model would
  fix this and also enable tracking a moving subject.
  A full `solvePnP`-style fit against a canonical 3D face model would be more
  accurate at extreme head angles.
- **No eye gaze or blink-driven animation**, even though the tracker computes
  blink amounts - most rigs don't have eyelid bones for it to drive. Wiring
  blink into glTF morph targets (and VRM's `BlendShapeProxy` when loading VRM
  models specifically) is the natural next step.
- **No calibration UI.** `FaceTracker::recalibrate()` exists (re-centers
  "neutral" head pose to whatever you're doing right now) but isn't bound to
  a hotkey in the app yet.
- **No virtual-camera/streaming output.** Right now EZVTB is a viewer window;
  piping the rendered avatar to a virtual webcam or OBS-friendly output is a
  reasonable next milestone once the tracking core is solid.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.
