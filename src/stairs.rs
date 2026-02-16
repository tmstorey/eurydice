// Stairs section: ascending corridor of finger-bone steps in darkness.

use bevy::prelude::*;

use crate::npc::NpcChevron;
use crate::player::{Player, PlayerLook};
use crate::sections::{PlotFlags, Sections};

pub struct StairsPlugin;

impl Plugin for StairsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Sections::Stairs), setup_stairs)
            .add_systems(OnExit(Sections::Stairs), exit_stairs)
            .add_systems(
                Update,
                (
                    stairs_movement,
                    stairs_chevron,
                    stairs_look_check,
                    stairs_exit,
                )
                    .chain()
                    .run_if(in_state(Sections::Stairs)),
            );
    }
}

const EYE_HEIGHT: f32 = 1.5;
const CORRIDOR_HALF_WIDTH: f32 = 3.0;
const CLAMP_MARGIN: f32 = 0.5;

const STEP_HEIGHT: f32 = 0.15;
const STEP_DEPTH: f32 = 1.0;
const NUM_STEPS: usize = 80;

const FINGER_PATH: &str = "character/finger.gltf";
/// Scale finger model down and widen to fit the corridor.
const FINGER_SCALE: f32 = 0.6;
const FINGER_X_SCALE: f32 = 1.1 / FINGER_SCALE;

/// Yaw delta (radians) from initial direction to count as "looked behind".
const LOOK_BEHIND_THRESHOLD: f32 = 2.6;

const CHEVRON_MARGIN: f32 = 40.0;

#[derive(Resource)]
struct StairsState {
    initial_yaw: f32,
}

#[derive(Component)]
struct StairStep;

fn setup_stairs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut player: Query<(&mut Transform, &mut PlayerLook), With<Player>>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.3, 0.25, 0.35),
        brightness: 3.0,
        affects_lightmapped_meshes: false,
    });

    let finger_scene: Handle<Scene> =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(FINGER_PATH));

    for i in 0..NUM_STEPS {
        let z = -(i as f32 * STEP_DEPTH);
        let y = i as f32 * STEP_HEIGHT;
        commands.spawn((
            StairStep,
            SceneRoot(finger_scene.clone()),
            Transform::from_xyz(0.0, y, z).with_scale(Vec3::new(
                FINGER_X_SCALE,
                FINGER_SCALE,
                FINGER_SCALE,
            )),
            DespawnOnExit(Sections::Stairs),
        ));
    }

    // Position player at the bottom of the stairs facing up (-Z).
    let initial_yaw;
    if let Ok((mut transform, mut look)) = player.single_mut() {
        look.yaw = 0.0;
        look.pitch = 0.0;
        transform.translation = Vec3::new(0.0, EYE_HEIGHT, STEP_DEPTH);
        transform.rotation = Quat::IDENTITY;
        initial_yaw = look.yaw;
    } else {
        initial_yaw = 0.0;
    }

    // Light at the top of the staircase.
    let top_y = (NUM_STEPS - 1) as f32 * STEP_HEIGHT;
    let top_z = -((NUM_STEPS - 1) as f32 * STEP_DEPTH);
    commands.spawn((
        PointLight {
            color: Color::srgb(0.8, 0.7, 1.0),
            intensity: 200_000.0,
            range: 150.0,
            ..default()
        },
        Transform::from_xyz(0.0, top_y + 5.0, top_z),
        DespawnOnExit(Sections::Stairs),
    ));

    commands.insert_resource(StairsState { initial_yaw });
}

fn stairs_movement(mut player: Query<&mut Transform, With<Player>>) {
    let Ok(mut transform) = player.single_mut() else {
        return;
    };

    // Clamp to corridor bounds.
    transform.translation.x = transform.translation.x.clamp(
        -(CORRIDOR_HALF_WIDTH - CLAMP_MARGIN),
        CORRIDOR_HALF_WIDTH - CLAMP_MARGIN,
    );

    let max_z = STEP_DEPTH + 1.0;
    let min_z = -((NUM_STEPS - 1) as f32 * STEP_DEPTH);
    transform.translation.z = transform.translation.z.clamp(min_z, max_z);

    // Snap Y to the current step height based on Z position.
    let progress = (-transform.translation.z / STEP_DEPTH).max(0.0);
    let step_y = progress.floor() * STEP_HEIGHT;
    transform.translation.y = step_y + EYE_HEIGHT;
}

/// Show the red chevron pointing toward "behind" (the start of the stairs).
fn stairs_chevron(
    mut chevron: Query<
        (&mut Node, &mut UiTransform, &mut TextColor, &mut Visibility),
        With<NpcChevron>,
    >,
    camera: Query<(&Camera, &GlobalTransform), With<Player>>,
) {
    let Ok((mut node, mut ui_transform, mut color, mut visibility)) = chevron.single_mut() else {
        return;
    };
    let Ok((camera, camera_global)) = camera.single() else {
        return;
    };

    *color = TextColor(Color::srgb(1.0, 0.0, 0.0));

    // "Behind" is back toward the start of the stairs (+Z from the player).
    let behind_point = camera_global.translation() + Vec3::Z * 20.0;

    let Some(viewport_size) = camera.logical_viewport_size() else {
        return;
    };
    let center = viewport_size / 2.0;

    let view_matrix = camera_global.affine().inverse();
    let behind_view = view_matrix.transform_point3(behind_point);

    let screen_pos = if behind_view.z < 0.0 {
        // "Behind" is in front of the camera (player turned around).
        camera
            .world_to_viewport(camera_global, behind_point)
            .unwrap_or(center)
    } else {
        // "Behind" is behind the camera (normal forward walking).
        let dir = Vec2::new(behind_view.x, behind_view.y).normalize_or_zero();
        dir * center.x.min(center.y) * 0.8 + center
    };

    let clamped_x = screen_pos
        .x
        .clamp(CHEVRON_MARGIN, viewport_size.x - CHEVRON_MARGIN);
    let clamped_y = screen_pos
        .y
        .clamp(CHEVRON_MARGIN, viewport_size.y - CHEVRON_MARGIN);
    node.left = Val::Px(clamped_x - 16.0);
    node.top = Val::Px(clamped_y - 16.0);

    // Rotate the chevron to point toward the behind-direction on screen.
    let dir = Vec2::new(screen_pos.x - center.x, screen_pos.y - center.y).normalize_or_zero();
    let angle = dir.y.atan2(dir.x);
    ui_transform.rotation = Rot2::radians(angle - std::f32::consts::FRAC_PI_2);

    *visibility = Visibility::Inherited;
}

fn stairs_look_check(
    player: Query<&PlayerLook, With<Player>>,
    state: Res<StairsState>,
    mut flags: ResMut<PlotFlags>,
) {
    if flags.player_looked_behind {
        return;
    }
    let Ok(look) = player.single() else {
        return;
    };

    // Compute the shortest angular distance between current and initial yaw.
    let delta = (look.yaw - state.initial_yaw).rem_euclid(std::f32::consts::TAU);
    let angle = if delta > std::f32::consts::PI {
        std::f32::consts::TAU - delta
    } else {
        delta
    };

    if angle > LOOK_BEHIND_THRESHOLD {
        flags.player_looked_behind = true;
    }
}

fn stairs_exit(
    player: Query<&Transform, With<Player>>,
    mut next_state: ResMut<NextState<Sections>>,
) {
    let Ok(transform) = player.single() else {
        return;
    };
    let top_z = -((NUM_STEPS - 2) as f32 * STEP_DEPTH);
    if transform.translation.z <= top_z {
        next_state.set(Sections::Awaken);
    }
}

fn exit_stairs(mut commands: Commands, mut chevron: Query<&mut Visibility, With<NpcChevron>>) {
    commands.insert_resource(GlobalAmbientLight::NONE);
    if let Ok(mut vis) = chevron.single_mut() {
        *vis = Visibility::Hidden;
    }
}
