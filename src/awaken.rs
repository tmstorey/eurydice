// Awaken section

use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use bevy::window::{CursorGrabMode, CursorOptions};

use crate::player::{Player, PlayerLook};
use crate::sections::{PlotFlags, Sections};

pub struct AwakenPlugin;

impl Plugin for AwakenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Sections::Awaken), setup_awaken)
            .add_systems(OnExit(Sections::Awaken), exit_awaken)
            .add_systems(Update, awaken_timer.run_if(in_state(Sections::Awaken)));
    }
}

const ROOM_PATH: &str = "room/room.gltf";
const NPC_PATH: &str = "character/character.gltf";
const ALT_PATH: &str = "character/base.gltf";
const ANIM_SITTING: usize = 26;
const EXIT_DELAY: f32 = 5.0;

#[derive(Resource)]
struct AwakenState {
    timer: f32,
}

#[derive(Resource)]
struct AwakenNpcAnimation {
    graph: Handle<AnimationGraph>,
    sitting: AnimationNodeIndex,
}

#[derive(Component)]
struct AwakenNpc;

fn setup_awaken(
    mut commands: Commands,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    asset_server: Res<AssetServer>,
    flags: Res<PlotFlags>,
    mut player: Query<(&mut Transform, &mut PlayerLook), With<Player>>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.9, 0.85, 0.7),
        brightness: 8.0,
        affects_lightmapped_meshes: false,
    });

    commands.insert_resource(AwakenState { timer: 0.0 });

    // Position camera facing +X
    if let Ok((mut transform, mut look)) = player.single_mut() {
        transform.translation = Vec3::new(0.0, 0.7, 0.0);
        look.yaw = -std::f32::consts::FRAC_PI_2;
        look.pitch = 0.0;
        transform.rotation = Quat::from_rotation_y(look.yaw);
    }

    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(ROOM_PATH))),
        DespawnOnExit(Sections::Awaken),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.5, 0.0)),
        DespawnOnExit(Sections::Awaken),
    ));

    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.9, 0.7),
            intensity: 100_000.0,
            range: 30.0,
            ..default()
        },
        Transform::from_xyz(0.0, 2.5, 0.0),
        DespawnOnExit(Sections::Awaken),
    ));

    // NPC in the chair, only if the player didn't look behind on the stairs
    if !flags.player_looked_behind {
        let mut graph = AnimationGraph::new();
        let path = if flags.chevron_count > 1 {
            NPC_PATH
        } else {
            ALT_PATH
        };
        let sitting = graph.add_clip(
            asset_server.load(GltfAssetLabel::Animation(ANIM_SITTING).from_asset(path)),
            1.0,
            graph.root,
        );
        commands.insert_resource(AwakenNpcAnimation {
            graph: graphs.add(graph),
            sitting,
        });

        commands
            .spawn((
                AwakenNpc,
                SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(path))),
                Transform::from_xyz(1.0, 0.0, 0.5)
                    .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
                DespawnOnExit(Sections::Awaken),
            ))
            .observe(start_sitting_animation);
    }
}

fn start_sitting_animation(
    trigger: On<SceneInstanceReady>,
    anim: Res<AwakenNpcAnimation>,
    mut commands: Commands,
    children: Query<&Children>,
    mut players: Query<(Entity, &mut AnimationPlayer)>,
) {
    for child in children.iter_descendants(trigger.entity) {
        if let Ok((anim_entity, mut player)) = players.get_mut(child) {
            player.play(anim.sitting).repeat();
            commands
                .entity(anim_entity)
                .insert(AnimationGraphHandle(anim.graph.clone()));
            break;
        }
    }
}

fn awaken_timer(
    mut state: ResMut<AwakenState>,
    time: Res<Time>,
    mut next_section: ResMut<NextState<Sections>>,
) {
    state.timer += time.delta_secs();
    if state.timer >= EXIT_DELAY {
        next_section.set(Sections::Menu);
    }
}

fn exit_awaken(mut commands: Commands, mut cursor: Query<&mut CursorOptions>) {
    commands.remove_resource::<AwakenState>();
    commands.remove_resource::<AwakenNpcAnimation>();
    commands.insert_resource(GlobalAmbientLight::NONE);

    let Ok(mut cursor) = cursor.single_mut() else {
        return;
    };
    cursor.grab_mode = CursorGrabMode::None;
    cursor.visible = true;
}
