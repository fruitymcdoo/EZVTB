//! Heuristic automatic bone assignment.
//!
//! Given the list of bone/joint names found in an arbitrary rigged model,
//! guess which [`HumanoidBone`] each one corresponds to. This is not meant
//! to be perfect — it is meant to get a usable starting map for the common
//! naming conventions in the wild (VRM / Unity Humanoid, Mixamo, and
//! Mixamo-derived rigs such as Ready Player Me) so a person can fix the few
//! bones it gets wrong instead of assigning all of them by hand.
//!
//! The result is always a [`crate::BoneMap`] that can be edited and saved.

use crate::bone::HumanoidBone;
use crate::config::BoneMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Laterality {
    Left,
    Right,
    None,
}

/// Split a raw bone name into lowercase tokens, breaking on non-alphanumeric
/// separators (`_`, `-`, `.`, `:`, `/`, whitespace) as well as camelCase and
/// digit boundaries. E.g. `"mixamorig:LeftForeArm"` -> `["mixamorig",
/// "left", "fore", "arm"]`, `"J_Bip_L_UpperArm"` -> `["j", "bip", "l",
/// "upper", "arm"]`.
fn tokenize(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut prev_kind: Option<CharKind> = None;

    #[derive(PartialEq, Clone, Copy)]
    enum CharKind {
        Lower,
        Upper,
        Digit,
    }

    let flush = |current: &mut String, tokens: &mut Vec<String>| {
        if !current.is_empty() {
            tokens.push(std::mem::take(current).to_lowercase());
        }
    };

    for c in name.chars() {
        if !c.is_alphanumeric() {
            flush(&mut current, &mut tokens);
            prev_kind = None;
            continue;
        }
        let kind = if c.is_ascii_digit() {
            CharKind::Digit
        } else if c.is_uppercase() {
            CharKind::Upper
        } else {
            CharKind::Lower
        };
        let boundary = matches!(
            (prev_kind, kind),
            (Some(CharKind::Lower), CharKind::Upper)
                | (Some(CharKind::Digit), CharKind::Upper | CharKind::Lower)
                | (Some(CharKind::Upper | CharKind::Lower), CharKind::Digit)
        );
        if boundary {
            flush(&mut current, &mut tokens);
        }
        current.push(c);
        prev_kind = Some(kind);
    }
    flush(&mut current, &mut tokens);
    tokens
}

/// Tokens that mark a rig-prefix / skeleton-tooling artifact rather than a
/// meaningful part of the bone's name. These are dropped before matching.
const NOISE_TOKENS: &[&str] = &[
    "mixamorig",
    "bip",
    "bip01",
    "biped",
    "valvebiped",
    "bone",
    "ctrl",
    "def",
    "j",
    "jnt",
    "joint",
    "armature",
    "skeleton",
    "humanoid",
    "root",
];

fn laterality_of(tokens: &[String]) -> Laterality {
    if tokens.iter().any(|t| t == "left" || t == "l" || t == "lt") {
        Laterality::Left
    } else if tokens.iter().any(|t| t == "right" || t == "r" || t == "rt") {
        Laterality::Right
    } else {
        Laterality::None
    }
}

/// A single candidate assignment of a source bone name to a humanoid slot,
/// scored by how specifically it matched.
struct Candidate {
    bone_index: usize,
    slot: HumanoidBone,
    score: i32,
}

fn score_simple(tokens: &[String], required_any: &[&str], excludes: &[&str]) -> Option<i32> {
    if excludes.iter().any(|ex| tokens.iter().any(|t| t == ex)) {
        return None;
    }
    if !tokens.iter().any(|t| required_any.contains(&t.as_str())) {
        return None;
    }
    // Fewer leftover tokens (after removing the matched keyword) means a
    // more specific / confident match.
    let matched = tokens
        .iter()
        .filter(|t| required_any.contains(&t.as_str()))
        .count() as i32;
    let extra = tokens.len() as i32 - matched;
    Some(100 - extra * 5)
}

/// Score candidate slots for one bone's tokens (with laterality/noise tokens
/// already stripped out of `tokens`, but `laterality` reported separately).
fn candidates_for_bone(tokens: &[String], laterality: Laterality) -> Vec<(HumanoidBone, i32)> {
    let mut out = Vec::new();

    // Central, non-lateral bones.
    if laterality == Laterality::None {
        if let Some(s) = score_simple(tokens, &["hips", "hip", "pelvis"], &[]) {
            out.push((HumanoidBone::Hips, s));
        }
        if let Some(s) = score_simple(tokens, &["neck"], &[]) {
            out.push((HumanoidBone::Neck, s));
        }
        if let Some(s) = score_simple(tokens, &["head"], &["top", "end", "eye"]) {
            out.push((HumanoidBone::Head, s));
        }
        if let Some(s) = score_simple(tokens, &["jaw", "chin"], &["end"]) {
            out.push((HumanoidBone::Jaw, s));
        }

        // Spine chain: Mixamo emits Spine, Spine1, Spine2 (occasionally
        // Spine3+); VRM/Unity only has three tiers (Spine, Chest,
        // UpperChest), so fold anything beyond the third tier into
        // UpperChest.
        if tokens.iter().any(|t| t == "spine") {
            let number: i32 = tokens
                .iter()
                .find_map(|t| t.parse::<i32>().ok())
                .unwrap_or(0);
            let slot = match number {
                0 => HumanoidBone::Spine,
                1 => HumanoidBone::Chest,
                _ => HumanoidBone::UpperChest,
            };
            out.push((slot, 100));
        }
        if let Some(s) = score_simple(tokens, &["chest"], &["upper"]) {
            out.push((HumanoidBone::Chest, s));
        }
        if tokens.iter().any(|t| t == "upper") && tokens.iter().any(|t| t == "chest") {
            out.push((HumanoidBone::UpperChest, 100));
        }
    }

    // Lateral (left/right) bones.
    if laterality != Laterality::None {
        let is_left = laterality == Laterality::Left;

        if let Some(s) = score_simple(tokens, &["eye"], &["brow", "lash", "lid", "glass"]) {
            out.push((if is_left { HumanoidBone::LeftEye } else { HumanoidBone::RightEye }, s));
        }
        if let Some(s) = score_simple(tokens, &["shoulder", "clavicle"], &[]) {
            out.push((
                if is_left { HumanoidBone::LeftShoulder } else { HumanoidBone::RightShoulder },
                s,
            ));
        }
        if tokens.iter().any(|t| t == "arm") {
            let is_lower = tokens.iter().any(|t| t == "fore" || t == "lower");
            let extra = tokens.len() as i32 - 1 - if is_lower { 1 } else { 0 };
            let score = 100 - extra * 5;
            let slot = match (is_left, is_lower) {
                (true, false) => HumanoidBone::LeftUpperArm,
                (true, true) => HumanoidBone::LeftLowerArm,
                (false, false) => HumanoidBone::RightUpperArm,
                (false, true) => HumanoidBone::RightLowerArm,
            };
            out.push((slot, score));
        }
        if let Some(s) = score_simple(tokens, &["hand"], &[]) {
            out.push((if is_left { HumanoidBone::LeftHand } else { HumanoidBone::RightHand }, s));
        }
        if tokens.iter().any(|t| t == "leg") {
            let is_upper = tokens.iter().any(|t| t == "up" || t == "upper");
            let is_lower = tokens.iter().any(|t| t == "lower" || t == "low" || t == "shin" || t == "calf");
            // Mixamo's bare "Leg" (no qualifier) is the shin, not the thigh.
            let slot = if is_upper {
                if is_left { HumanoidBone::LeftUpperLeg } else { HumanoidBone::RightUpperLeg }
            } else if is_lower {
                if is_left { HumanoidBone::LeftLowerLeg } else { HumanoidBone::RightLowerLeg }
            } else if is_left {
                HumanoidBone::LeftLowerLeg
            } else {
                HumanoidBone::RightLowerLeg
            };
            out.push((slot, 90));
        }
        if let Some(s) = score_simple(tokens, &["foot"], &[]) {
            out.push((if is_left { HumanoidBone::LeftFoot } else { HumanoidBone::RightFoot }, s));
        }
        if let Some(s) = score_simple(tokens, &["toe", "toes"], &[]) {
            out.push((if is_left { HumanoidBone::LeftToes } else { HumanoidBone::RightToes }, s));
        }
    }

    out
}

/// Guess a [`BoneMap`] from the raw bone/joint names present in a loaded
/// model's skeleton. `bone_names` should be every joint name exactly as it
/// appears in the model (order doesn't matter).
///
/// Each source name is assigned to at most one [`HumanoidBone`] slot, and
/// each slot receives at most one source name — ambiguous or unrecognized
/// bones (fingers, spring bones, accessories, ...) are simply left out, to
/// be filled in by hand if needed.
pub fn auto_map_bones(bone_names: &[String]) -> BoneMap {
    let per_bone: Vec<(Vec<String>, Laterality)> = bone_names
        .iter()
        .map(|name| {
            let raw_tokens = tokenize(name);
            let laterality = laterality_of(&raw_tokens);
            let filtered: Vec<String> = raw_tokens
                .into_iter()
                .filter(|t| {
                    !NOISE_TOKENS.contains(&t.as_str())
                        && !(laterality == Laterality::Left && (t == "left" || t == "l" || t == "lt"))
                        && !(laterality == Laterality::Right && (t == "right" || t == "r" || t == "rt"))
                })
                .collect();
            (filtered, laterality)
        })
        .collect();

    let mut candidates: Vec<Candidate> = Vec::new();
    for (bone_index, (tokens, laterality)) in per_bone.iter().enumerate() {
        for (slot, score) in candidates_for_bone(tokens, *laterality) {
            candidates.push(Candidate { bone_index, slot, score });
        }
    }

    // Highest-confidence matches win first; ties broken by shorter (more
    // specific) source name so e.g. "LeftHand" beats "LeftHandIndex1" for
    // the LeftHand slot even if both scored equally.
    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| bone_names[a.bone_index].len().cmp(&bone_names[b.bone_index].len()))
    });

    let mut map = BoneMap::default();
    let mut slot_taken = std::collections::HashSet::new();
    let mut bone_taken = std::collections::HashSet::new();

    for c in candidates {
        if slot_taken.contains(&c.slot) || bone_taken.contains(&c.bone_index) {
            continue;
        }
        map.set(c.slot, bone_names[c.bone_index].clone());
        slot_taken.insert(c.slot);
        bone_taken.insert(c.bone_index);
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn maps_vrm_unity_humanoid_names() {
        let bones = names(&[
            "Hips", "Spine", "Chest", "UpperChest", "Neck", "Head", "LeftEye", "RightEye", "Jaw",
            "LeftShoulder", "LeftUpperArm", "LeftLowerArm", "LeftHand", "RightShoulder",
            "RightUpperArm", "RightLowerArm", "RightHand", "LeftUpperLeg", "LeftLowerLeg",
            "LeftFoot", "RightUpperLeg", "RightLowerLeg", "RightFoot",
        ]);
        let map = auto_map_bones(&bones);
        for name in &bones {
            let expected: HumanoidBone = HumanoidBone::ALL
                .iter()
                .copied()
                .find(|b| b.as_str() == name.as_str())
                .unwrap();
            assert_eq!(map.get(expected), Some(name.as_str()), "slot {expected}");
        }
    }

    #[test]
    fn maps_mixamo_names() {
        let bones = names(&[
            "mixamorig:Hips",
            "mixamorig:Spine",
            "mixamorig:Spine1",
            "mixamorig:Spine2",
            "mixamorig:Neck",
            "mixamorig:Head",
            "mixamorig:LeftShoulder",
            "mixamorig:LeftArm",
            "mixamorig:LeftForeArm",
            "mixamorig:LeftHand",
            "mixamorig:LeftHandIndex1",
            "mixamorig:LeftHandIndex2",
            "mixamorig:RightUpLeg",
            "mixamorig:RightLeg",
            "mixamorig:RightFoot",
            "mixamorig:RightToeBase",
        ]);
        let map = auto_map_bones(&bones);
        assert_eq!(map.get(HumanoidBone::Hips), Some("mixamorig:Hips"));
        assert_eq!(map.get(HumanoidBone::Spine), Some("mixamorig:Spine"));
        assert_eq!(map.get(HumanoidBone::Chest), Some("mixamorig:Spine1"));
        assert_eq!(map.get(HumanoidBone::UpperChest), Some("mixamorig:Spine2"));
        assert_eq!(map.get(HumanoidBone::Neck), Some("mixamorig:Neck"));
        assert_eq!(map.get(HumanoidBone::Head), Some("mixamorig:Head"));
        assert_eq!(map.get(HumanoidBone::LeftShoulder), Some("mixamorig:LeftShoulder"));
        assert_eq!(map.get(HumanoidBone::LeftUpperArm), Some("mixamorig:LeftArm"));
        assert_eq!(map.get(HumanoidBone::LeftLowerArm), Some("mixamorig:LeftForeArm"));
        // The wrist itself must win over the (longer, more specific) finger
        // bone names for the LeftHand slot.
        assert_eq!(map.get(HumanoidBone::LeftHand), Some("mixamorig:LeftHand"));
        assert_eq!(map.get(HumanoidBone::RightUpperLeg), Some("mixamorig:RightUpLeg"));
        assert_eq!(map.get(HumanoidBone::RightLowerLeg), Some("mixamorig:RightLeg"));
        assert_eq!(map.get(HumanoidBone::RightFoot), Some("mixamorig:RightFoot"));
        assert_eq!(map.get(HumanoidBone::RightToes), Some("mixamorig:RightToeBase"));
    }

    #[test]
    fn ignores_unrelated_bones() {
        let bones = names(&["Head", "HeadphoneAttach", "Ponytail_01", "Cloth_Back_02"]);
        let map = auto_map_bones(&bones);
        assert_eq!(map.get(HumanoidBone::Head), Some("Head"));
        assert_eq!(map.iter().count(), 1);
    }
}
