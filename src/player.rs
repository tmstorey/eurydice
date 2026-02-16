use std::f32::consts::PI;
use std::time::Duration;

// First-person camera controller with mouse look and keyboard movement.
use crate::dream::DreamSettings;
use crate::sections::Sections;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use bevy::window::{CursorGrabMode, CursorOptions};
use bevy::{camera::Exposure, post_process::bloom::Bloom};
#[cfg(not(target_arch = "wasm32"))]
use bevy::{
    light::AtmosphereEnvironmentMapLight,
    pbr::{Atmosphere, AtmosphereSettings, ScatteringMedium},
};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_player, load_arm_assets).chain())
            .insert_resource(ClearColor(Color::BLACK))
            .insert_resource(GlobalAmbientLight::NONE)
            .add_systems(
                Update,
                (toggle_cursor_grab, mouse_look, player_movement).run_if(
                    in_state(Sections::Chase)
                        .or(in_state(Sections::Underworld))
                        .or(in_state(Sections::Stairs)),
                ),
            )
            .add_systems(
                OnEnter(Sections::Chase),
                (reset_player, spawn_chase_light, set_sky_background),
            )
            .add_systems(
                OnEnter(Sections::Underworld),
                (spawn_torch_arms, set_black_background),
            )
            .add_systems(
                OnEnter(Sections::Awaken),
                (despawn_arms, set_sky_background),
            );
    }
}

#[derive(Component)]
pub struct Player;

/// Tracks the player's yaw and pitch for composed camera rotation.
#[derive(Component)]
pub struct PlayerLook {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Resource)]
pub struct ArmAssets {
    pub scene: Handle<Scene>,
    pub graph: Handle<AnimationGraph>,
    pub torch: AnimationNodeIndex,
}

#[derive(Component)]
pub struct PlayerArms;

const EYE_HEIGHT: f32 = 1.5;
const MOUSE_SENSITIVITY: f32 = 0.003;
const MOVE_SPEED: f32 = 10.0;
const MAX_PITCH: f32 = 1.3;

pub const SKY_BLUE: Color = Color::linear_rgb(0.53, 0.81, 0.92);

fn spawn_player(
    mut commands: Commands,
    #[cfg(not(target_arch = "wasm32"))] mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
) {
    #[allow(unused_variables)]
    let camera = commands
        .spawn((
            Player,
            PlayerLook {
                yaw: 0.0,
                pitch: 0.0,
            },
            Camera3d::default(),
            Projection::from(PerspectiveProjection {
                fov: std::f32::consts::FRAC_PI_2 * 0.8,
                near: 0.01,
                ..default()
            }),
            Exposure { ev100: 10.0 },
            Bloom::NATURAL,
            Transform::from_xyz(0.0, 10.0, 0.0),
            DreamSettings {
                intensity: 0.0,
                time: 0.0,
                _align: 0.0,
                _align2: 0.0,
            },
        ))
        .id();

    #[cfg(not(target_arch = "wasm32"))]
    commands.entity(camera).insert((
        Atmosphere::earthlike(scattering_mediums.add(ScatteringMedium::default())),
        AtmosphereSettings::default(),
        AtmosphereEnvironmentMapLight::default(),
    ));
}

fn toggle_cursor_grab(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut cursor: Query<&mut CursorOptions>,
) {
    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };

    if mouse.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn mouse_look(
    mut motion: MessageReader<MouseMotion>,
    mut query: Query<(&mut Transform, &mut PlayerLook), With<Player>>,
    cursor: Query<&CursorOptions>,
) {
    let Ok(cursor) = cursor.single() else {
        return;
    };
    if cursor.grab_mode != CursorGrabMode::Locked {
        return;
    }

    let mut delta = Vec2::ZERO;
    for ev in motion.read() {
        delta += ev.delta;
    }
    if delta == Vec2::ZERO {
        return;
    }

    let Ok((mut transform, mut look)) = query.single_mut() else {
        return;
    };
    look.yaw -= delta.x * MOUSE_SENSITIVITY;
    look.pitch = (look.pitch - delta.y * MOUSE_SENSITIVITY).clamp(-MAX_PITCH, MAX_PITCH);
    transform.rotation = Quat::from_rotation_y(look.yaw) * Quat::from_rotation_x(look.pitch);
}

fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Player>>,
    time: Res<Time>,
    section: Res<State<Sections>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    let forward = *transform.forward();
    let forward_xz = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();

    let mut movement = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        movement += forward_xz;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        movement -= forward_xz;
    }

    let move_speed = match **section {
        Sections::Chase => MOVE_SPEED,
        _ => MOVE_SPEED / 2.0,
    };

    transform.translation += movement * move_speed * time.delta_secs();
}

const ARMS_6F_PATH: &str = "character/arms-6finger.gltf";

// Idle_Torch_Loop animation index
const ANIM_TORCH: usize = 10;

fn load_arm_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let mut graph = AnimationGraph::new();
    let torch = graph.add_clip(
        asset_server.load(GltfAssetLabel::Animation(ANIM_TORCH).from_asset(ARMS_6F_PATH)),
        1.0,
        graph.root,
    );
    commands.insert_resource(ArmAssets {
        scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset(ARMS_6F_PATH)),
        graph: graphs.add(graph),
        torch,
    });
}

fn spawn_torch_arms(
    mut commands: Commands,
    player: Query<Entity, With<Player>>,
    assets: Res<ArmAssets>,
) {
    let Ok(player_entity) = player.single() else {
        return;
    };
    commands.entity(player_entity).with_children(|parent| {
        parent
            .spawn((
                PlayerArms,
                SceneRoot(assets.scene.clone()),
                Transform::from_xyz(0.0, -0.1 - EYE_HEIGHT, -0.19)
                    .with_rotation(Quat::from_rotation_y(PI)),
            ))
            .observe(start_torch_animation);
    });
}

fn start_torch_animation(
    trigger: On<SceneInstanceReady>,
    assets: Res<ArmAssets>,
    mut commands: Commands,
    children: Query<&Children>,
    mut players: Query<(Entity, &mut AnimationPlayer)>,
    names: Query<&Name>,
) {
    let entity = trigger.entity;
    for child in children.iter_descendants(entity) {
        // Pause the torch animation on its first frame.
        if let Ok((anim_entity, mut player)) = players.get_mut(child) {
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, assets.torch, Duration::ZERO)
                .seek_to(0.0)
                .pause();
            commands
                .entity(anim_entity)
                .insert(AnimationGraphHandle(assets.graph.clone()))
                .insert(transitions);
        }

        // Spawn a point light at the candle's Empty node.
        if names.get(child).is_ok_and(|n| n.as_str() == "Empty") {
            commands.entity(child).with_children(|parent| {
                parent.spawn(PointLight {
                    color: Color::linear_rgb(1.0, 0.7, 0.3),
                    intensity: 50_000.0,
                    range: 120.0,
                    ..default()
                });
            });
        }
    }
}

fn despawn_arms(mut commands: Commands, arms: Query<Entity, With<PlayerArms>>) {
    if let Ok(entity) = arms.single() {
        commands.entity(entity).despawn();
    }
}

fn reset_player(
    mut query: Query<(&mut Transform, &mut PlayerLook, &mut DreamSettings), With<Player>>,
) {
    let Ok((mut transform, mut look, mut dream)) = query.single_mut() else {
        return;
    };
    transform.translation = Vec3::new(0.0, 10.0, 0.0);
    look.yaw = 0.0;
    look.pitch = 0.0;
    transform.rotation = Quat::IDENTITY;
    dream.intensity = 0.0;
    dream.time = 0.0;
}

fn spawn_chase_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.5, 0.0)),
    ));
}

fn set_black_background(mut clear_color: ResMut<ClearColor>) {
    clear_color.0 = Color::BLACK;
}

fn set_sky_background(mut clear_color: ResMut<ClearColor>) {
    clear_color.0 = SKY_BLUE;
}
