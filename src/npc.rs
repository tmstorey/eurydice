// NPC that leads the player across the terrain, demonstrating terrain changes.
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use rand::Rng;

use crate::player::Player;
use crate::sections::{PlotFlags, Sections};
use crate::terrain::generation::NoiseSampler;
use crate::terrain::{StaleChunk, TerrainConfig, TerrainNoise, terrain_height};

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_npc_assets, spawn_npc_chevron).chain())
            .add_systems(OnEnter(Sections::Chase), spawn_npc)
            .add_systems(
                Update,
                (npc_ai, npc_movement, npc_terrain_follow, update_npc_chevron)
                    .chain()
                    .run_if(in_state(Sections::Chase)),
            );
    }
}

const NPC_PATH: &str = "character/character.gltf";

// Animation indices (alphabetical order in the GLTF)
const ANIM_IDLE: usize = 8; // Idle_Loop
const ANIM_JOG: usize = 15; // Jog_Fwd_Loop
const ANIM_SPRINT: usize = 31; // Sprint_Loop

const SPRINT_SPEED: f32 = 9.8;
const WAYPOINT_REACHED_DIST: f32 = 2.0;
const CIRCLE_ENTER_DIST: f32 = 8.0;
const CIRCLE_EXIT_DIST: f32 = 32.0;
const CIRCLE_RADIUS: f32 = 8.0;
const CIRCLE_SPEED: f32 = 1.0; // radians per second
const WAYPOINT_MIN_DIST: f32 = 24.0;
const WAYPOINT_MAX_DIST: f32 = 48.0;
/// Max turn angle when picking a new waypoint (90 degrees).
const MAX_TURN: f32 = std::f32::consts::FRAC_PI_2;
const IDLE_DIST: f32 = 128.0;
const CHEVRON_SHOW_DIST: f32 = 32.0;
const CHEVRON_MARGIN: f32 = 40.0;

#[derive(Component)]
pub struct Npc;

#[derive(Component)]
struct NpcTarget(Vec2);

#[derive(Component)]
enum NpcState {
    Idle,
    Wandering,
    Circling { angle: f32 },
}

#[derive(Component)]
struct NpcHeading(f32);

/// Stores the animation graph and node indices for the NPC.
#[derive(Component)]
struct NpcAnimations {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    jog: AnimationNodeIndex,
    sprint: AnimationNodeIndex,
}

#[derive(Resource)]
struct NpcAssets {
    scene: Handle<Scene>,
    animations: NpcAnimations,
}

fn load_npc_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let mut graph = AnimationGraph::new();
    let idle = graph.add_clip(
        asset_server.load(GltfAssetLabel::Animation(ANIM_IDLE).from_asset(NPC_PATH)),
        1.0,
        graph.root,
    );
    let jog = graph.add_clip(
        asset_server.load(GltfAssetLabel::Animation(ANIM_JOG).from_asset(NPC_PATH)),
        1.0,
        graph.root,
    );
    let sprint = graph.add_clip(
        asset_server.load(GltfAssetLabel::Animation(ANIM_SPRINT).from_asset(NPC_PATH)),
        1.0,
        graph.root,
    );

    let graph_handle = graphs.add(graph);

    commands.insert_resource(NpcAssets {
        scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset(NPC_PATH)),
        animations: NpcAnimations {
            graph: graph_handle,
            idle,
            jog,
            sprint,
        },
    });
}

fn spawn_npc(mut commands: Commands, assets: Res<NpcAssets>) {
    // Spawn ahead of the player start position (player starts at 0, 10, 0 facing -Z)
    let initial_heading = std::f32::consts::PI; // facing -Z
    commands
        .spawn((
            Npc,
            NpcState::Wandering,
            NpcTarget(Vec2::new(0.0, -30.0)),
            NpcHeading(initial_heading),
            SceneRoot(assets.scene.clone()),
            Transform::from_xyz(0.0, 10.0, -12.0),
        ))
        .observe(start_animation);
}

fn start_animation(
    _trigger: On<SceneInstanceReady>,
    npc_assets: Res<NpcAssets>,
    mut commands: Commands,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let entity = _trigger.entity;
    for child in children.iter_descendants(entity) {
        if let Ok(mut player) = players.get_mut(child) {
            player.play(npc_assets.animations.sprint).repeat();
            commands
                .entity(child)
                .insert(AnimationGraphHandle(npc_assets.animations.graph.clone()));
            break;
        }
    }
}

fn npc_ai(
    mut npc_query: Query<(&Transform, &mut NpcState, &mut NpcTarget, &mut NpcHeading), With<Npc>>,
    player_query: Query<&Transform, With<Player>>,
    npc_assets: Res<NpcAssets>,
    children: Query<&Children>,
    npc_entities: Query<Entity, With<Npc>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok((npc_transform, mut state, mut target, mut heading)) = npc_query.single_mut() else {
        return;
    };

    let npc_pos = Vec2::new(npc_transform.translation.x, npc_transform.translation.z);
    let player_pos = Vec2::new(
        player_transform.translation.x,
        player_transform.translation.z,
    );
    let dist_to_player = npc_pos.distance(player_pos);

    let mut switch_animation = None;

    match *state {
        NpcState::Idle => {
            if dist_to_player < IDLE_DIST {
                target.0 = pick_waypoint(npc_pos, heading.0);
                *state = NpcState::Wandering;
                switch_animation = Some(npc_assets.animations.sprint);
            }
        }
        NpcState::Wandering => {
            if dist_to_player > IDLE_DIST {
                *state = NpcState::Idle;
                switch_animation = Some(npc_assets.animations.idle);
            } else if dist_to_player < CIRCLE_ENTER_DIST {
                let offset = npc_pos - player_pos;
                let angle = offset.y.atan2(offset.x);
                *state = NpcState::Circling { angle };
                switch_animation = Some(npc_assets.animations.jog);
            } else {
                let dist_to_target = npc_pos.distance(target.0);
                if dist_to_target < WAYPOINT_REACHED_DIST {
                    target.0 = pick_waypoint(npc_pos, heading.0);
                }
            }
        }
        NpcState::Circling { .. } => {
            if dist_to_player > CIRCLE_EXIT_DIST {
                let away = (npc_pos - player_pos).normalize_or_zero();
                heading.0 = away.y.atan2(away.x);
                target.0 = pick_waypoint(npc_pos, heading.0);
                *state = NpcState::Wandering;
                switch_animation = Some(npc_assets.animations.sprint);
            }
        }
    }

    // Switch animation if state changed
    if let Some(anim_index) = switch_animation {
        if let Ok(npc_entity) = npc_entities.single() {
            for child in children.iter_descendants(npc_entity) {
                if let Ok(mut player) = players.get_mut(child) {
                    player.stop_all();
                    player.play(anim_index).repeat();
                    break;
                }
            }
        }
    }
}

fn npc_movement(
    mut query: Query<(&mut Transform, &mut NpcState, &NpcTarget, &mut NpcHeading), With<Npc>>,
    player_query: Query<&Transform, (With<Player>, Without<Npc>)>,
    time: Res<Time>,
) {
    let Ok((mut transform, mut state, target, mut heading)) = query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    let npc_pos = Vec2::new(transform.translation.x, transform.translation.z);

    match *state {
        NpcState::Idle => {}
        NpcState::Wandering => {
            let dir = (target.0 - npc_pos).normalize_or_zero();
            if dir != Vec2::ZERO {
                heading.0 = dir.y.atan2(dir.x);
                let movement = dir * SPRINT_SPEED * dt;
                transform.translation.x += movement.x;
                transform.translation.z += movement.y;
                // Face movement direction (Bevy's forward is -Z, so rotate accordingly)
                transform.rotation =
                    Quat::from_rotation_y(-heading.0 + std::f32::consts::FRAC_PI_2);
            }
        }
        NpcState::Circling { ref mut angle } => {
            let Ok(player_transform) = player_query.single() else {
                return;
            };
            let player_pos = Vec2::new(
                player_transform.translation.x,
                player_transform.translation.z,
            );

            *angle += CIRCLE_SPEED * dt;
            let circle_pos = player_pos + Vec2::new(angle.cos(), angle.sin()) * CIRCLE_RADIUS;
            transform.translation.x = circle_pos.x;
            transform.translation.z = circle_pos.y;
            // Face tangent to the circle (perpendicular to the radius).
            let tangent_angle = *angle + std::f32::consts::FRAC_PI_2;
            heading.0 = tangent_angle;
            transform.rotation = Quat::from_rotation_y(-heading.0 + std::f32::consts::FRAC_PI_2);
        }
    }
}

fn npc_terrain_follow(
    mut query: Query<&mut Transform, With<Npc>>,
    noise: Res<TerrainNoise>,
    config: Res<TerrainConfig>,
    sampler: Res<NoiseSampler>,
    stale: Res<StaleChunk>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };
    let height = terrain_height(
        transform.translation.x,
        transform.translation.z,
        &noise,
        &sampler,
        config.amplitude,
        config.noise_scale,
        config.chunk_size,
        stale.0.as_ref(),
    );
    transform.translation.y = height;
}

#[derive(Component)]
pub struct NpcChevron;

fn spawn_npc_chevron(mut commands: Commands) {
    commands.spawn((
        NpcChevron,
        Text::new("v"),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn update_npc_chevron(
    mut chevron: Query<(&mut Node, &mut UiTransform, &mut Visibility), With<NpcChevron>>,
    npc_query: Query<&GlobalTransform, With<Npc>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<Player>>,
    mut flags: ResMut<PlotFlags>,
) {
    let Ok((mut node, mut chevron_transform, mut visibility)) = chevron.single_mut() else {
        return;
    };
    let Ok(npc_global) = npc_query.single() else {
        *visibility = Visibility::Hidden;
        return;
    };
    let Ok((camera, camera_global)) = camera_query.single() else {
        return;
    };

    // Aim at the NPC's torso rather than feet.
    let npc_world = npc_global.translation() + Vec3::Y * 4.0;
    let cam_pos = camera_global.translation();
    let dist = Vec2::new(npc_world.x - cam_pos.x, npc_world.z - cam_pos.z).length();

    let Some(viewport_size) = camera.logical_viewport_size() else {
        return;
    };
    let center = viewport_size / 2.0;

    // Transform NPC position into camera view space to check if in front or behind.
    let view_matrix = camera_global.affine().inverse();
    let npc_view = view_matrix.transform_point3(npc_world);

    // In Bevy's view space, camera looks down -Z, so npc_view.z < 0 means in front.
    let screen_pos = if npc_view.z < 0.0 {
        // NPC is in front of camera - project to screen
        if dist < CHEVRON_SHOW_DIST {
            *visibility = Visibility::Hidden;
            return;
        }
        if let Ok(vp) = camera.world_to_viewport(camera_global, npc_world) {
            vp
        } else {
            center
        }
    } else {
        // NPC is behind camera - flip the direction so chevron points correctly
        Vec2::new(npc_view.x, npc_view.y).normalize_or_zero() * center.x.min(center.y) + center
    };

    if npc_view.z < 0.0 {
        // NPC is in front - place chevron at projected position, no rotation.
        let clamped_x = screen_pos
            .x
            .clamp(CHEVRON_MARGIN, viewport_size.x - CHEVRON_MARGIN);
        let clamped_y = screen_pos
            .y
            .clamp(CHEVRON_MARGIN, viewport_size.y - CHEVRON_MARGIN);
        node.left = Val::Px(clamped_x - 16.0);
        node.top = Val::Px(clamped_y - 16.0);
        chevron_transform.rotation = Rot2::IDENTITY;
    } else {
        // NPC is behind - place chevron partway from center toward the edge, rotated.
        let dir = (screen_pos - center).normalize_or_zero();
        let edge_dist = center.x.min(center.y) * 0.5;
        let pos = center + dir * edge_dist;
        node.left = Val::Px(pos.x - 16.0);
        node.top = Val::Px(pos.y - 16.0);
        let angle = dir.y.atan2(dir.x);
        chevron_transform.rotation = Rot2::radians(angle - std::f32::consts::FRAC_PI_2);
    }

    if *visibility == Visibility::Hidden {
        flags.chevron_count += 1;
    }

    *visibility = Visibility::Inherited;
}

/// Pick a random waypoint within MAX_TURN of the current heading, at a distance
/// between WAYPOINT_MIN_DIST and WAYPOINT_MAX_DIST.
fn pick_waypoint(pos: Vec2, heading: f32) -> Vec2 {
    let mut rng = rand::rng();
    let turn: f32 = rng.random_range(-MAX_TURN..=MAX_TURN);
    let dist: f32 = rng.random_range(WAYPOINT_MIN_DIST..=WAYPOINT_MAX_DIST);
    let angle = heading + turn;
    pos + Vec2::new(angle.cos(), angle.sin()) * dist
}
