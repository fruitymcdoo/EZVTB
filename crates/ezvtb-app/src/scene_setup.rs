use bevy::prelude::*;

/// A simple fixed camera framing roughly where a humanoid avatar's upper
/// body ends up after glTF import, plus enough light to see it. Good
/// enough to confirm tracking is driving the model; a proper orbit camera
/// is a natural follow-up (see roadmap).
pub fn spawn_camera_and_lights(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.4, 2.2).looking_at(Vec3::new(0.0, 1.3, 0.0), Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight { illuminance: 6000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(2.0, 4.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(GlobalAmbientLight { brightness: 300.0, ..default() });
}
