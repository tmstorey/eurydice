// Underworld section

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;
use noiz::prelude::*;

use crate::player::{Player, PlayerLook};
use crate::sections::Sections;
use crate::terrain::TerrainNoise;

pub struct UnderworldPlugin;

impl Plugin for UnderworldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Sections::Underworld), setup_underworld)
            .add_systems(OnExit(Sections::Underworld), exit_underworld)
            .add_systems(
                Update,
                (
                    underworld_terrain_follow,
                    underworld_pool_check,
                    underworld_npc_rotate,
                )
                    .chain()
                    .run_if(in_state(Sections::Underworld)),
            );
    }
}

const EYE_HEIGHT: f32 = 1.5;

// Corridor geometry.
const CORRIDOR_HALF_WIDTH: f32 = 3.0;
const CORRIDOR_LENGTH: f32 = 100.0;
const WALL_HEIGHT: f32 = 20.0;
const WALL_WIDTH: f32 = 3.0;
const MESH_HALF_WIDTH: f32 = CORRIDOR_HALF_WIDTH + WALL_WIDTH;
const FLOOR_AMPLITUDE: f32 = 1.0;
const NOISE_SCALE: f32 = 0.05;
const MESH_STEP: f32 = 0.5;
const CLAMP_MARGIN: f32 = 0.5;

// Pool and NPC.
const POOL_Z: f32 = -90.0;
const POOL_SIZE: f32 = 4.0;
const POOL_TRIGGER_DIST: f32 = 5.0;
const POOL_TRIGGER_PITCH: f32 = -0.5;
const NPC_ROTATION_DURATION: f32 = 3.0;
const NPC_WAIT_DURATION: f32 = 3.0;
const POOL_DEPTH: f32 = 5.0;
const POOL_BLEND: f32 = 3.0;

const NPC_PATH: &str = "character/character.gltf";
const ANIM_TORCH: usize = 10;

#[derive(Component)]
struct UnderworldNpc;

#[derive(Resource)]
struct UnderworldNpcAnimation {
    graph: Handle<AnimationGraph>,
    torch: AnimationNodeIndex,
}

#[derive(Resource)]
struct UnderworldState {
    phase: UnderworldPhase,
    timer: f32,
}

enum UnderworldPhase {
    Walking,
    Rotating,
    Waiting,
}

fn base_floor_height(wx: f32, wz: f32, noise: &TerrainNoise) -> f32 {
    let p = Vec3::new(wx * NOISE_SCALE, 0.0, wz * NOISE_SCALE);
    noise.0.sample_for::<f32>(p) * FLOOR_AMPLITUDE
}

fn corridor_floor_height(wx: f32, wz: f32, noise: &TerrainNoise) -> f32 {
    let base = base_floor_height(wx, wz, noise);
    // Depress the floor around the pool so terrain doesn't clip the water.
    let dx = wx;
    let dz = wz - POOL_Z;
    let dist = (dx * dx + dz * dz).sqrt();
    let pool_radius = POOL_SIZE * 0.5 + POOL_BLEND;
    if dist < pool_radius {
        let t = (1.0 - dist / pool_radius).max(0.0);
        base - t * t * POOL_DEPTH
    } else {
        base
    }
}

fn wall_curve(abs_x: f32) -> f32 {
    if abs_x <= CORRIDOR_HALF_WIDTH {
        0.0
    } else {
        let t = (abs_x - CORRIDOR_HALF_WIDTH) / WALL_WIDTH;
        t * t * WALL_HEIGHT
    }
}

/// Wall ramp based on proximity to the nearest z-boundary.
fn end_wall_curve(wz: f32) -> f32 {
    let dist_front = -wz;
    let dist_back = wz + CORRIDOR_LENGTH;
    let nearest = dist_front.min(dist_back).max(0.0);
    if nearest >= WALL_WIDTH {
        0.0
    } else {
        let t = 1.0 - nearest / WALL_WIDTH;
        t * t * WALL_HEIGHT
    }
}

fn corridor_height(wx: f32, wz: f32, noise: &TerrainNoise) -> f32 {
    corridor_floor_height(wx, wz, noise) + wall_curve(wx.abs()) + end_wall_curve(wz)
}

fn generate_corridor_mesh(noise: &TerrainNoise) -> Mesh {
    let width = MESH_HALF_WIDTH * 2.0;
    let res_x = (width / MESH_STEP) as usize + 1;
    let res_z = (CORRIDOR_LENGTH / MESH_STEP) as usize + 1;

    let mut positions = Vec::with_capacity(res_x * res_z);
    let mut normals = Vec::with_capacity(res_x * res_z);
    let mut indices = Vec::new();

    for zi in 0..res_z {
        for xi in 0..res_x {
            let wx = (xi as f32 * MESH_STEP) - MESH_HALF_WIDTH;
            let wz = -(zi as f32 * MESH_STEP);
            let height = corridor_height(wx, wz, noise);
            positions.push([wx, height, wz]);

            // Central-difference normals.
            let eps = MESH_STEP * 0.5;
            let normal = Vec3::new(
                corridor_height(wx - eps, wz, noise) - corridor_height(wx + eps, wz, noise),
                2.0 * eps,
                corridor_height(wx, wz - eps, noise) - corridor_height(wx, wz + eps, noise),
            )
            .normalize();
            normals.push(normal.to_array());
        }
    }

    for zi in 0..(res_z - 1) {
        for xi in 0..(res_x - 1) {
            let i = (zi * res_x + xi) as u32;
            let w = res_x as u32;
            indices.push(i);
            indices.push(i + 1);
            indices.push(i + w);
            indices.push(i + 1);
            indices.push(i + w + 1);
            indices.push(i + w);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn setup_underworld(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    noise: Res<TerrainNoise>,
    asset_server: Res<AssetServer>,
    mut player: Query<(&mut Transform, &mut PlayerLook), With<Player>>,
) {
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.4, 0.35, 0.5),
        brightness: 5.0,
        affects_lightmapped_meshes: false,
    });

    commands.insert_resource(UnderworldState {
        phase: UnderworldPhase::Walking,
        timer: 0.0,
    });

    // Load NPC torch animation.
    let mut graph = AnimationGraph::new();
    let torch = graph.add_clip(
        asset_server.load(GltfAssetLabel::Animation(ANIM_TORCH).from_asset(NPC_PATH)),
        1.0,
        graph.root,
    );
    commands.insert_resource(UnderworldNpcAnimation {
        graph: graphs.add(graph),
        torch,
    });

    // Position player at corridor entrance facing north (-Z), past the front wall.
    if let Ok((mut transform, mut look)) = player.single_mut() {
        let spawn_z = -(WALL_WIDTH + 2.0);
        let floor_y = corridor_floor_height(0.0, spawn_z, &noise);
        transform.translation = Vec3::new(0.0, floor_y + EYE_HEIGHT, spawn_z);
        look.yaw = 0.0;
        look.pitch = 0.0;
        transform.rotation = Quat::IDENTITY;
    }

    // Corridor mesh.
    let corridor_mesh = generate_corridor_mesh(&noise);
    let corridor_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.28, 0.22),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(corridor_mesh)),
        MeshMaterial3d(corridor_material),
        DespawnOnExit(Sections::Underworld),
    ));

    // Pool surface.
    let pool_y = base_floor_height(0.0, POOL_Z, &noise) - 1.5;
    let pool_material = materials.add(StandardMaterial {
        base_color: Color::linear_rgba(0.02, 0.02, 0.08, 0.6),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.1,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(POOL_SIZE, POOL_SIZE))),
        MeshMaterial3d(pool_material),
        Transform::from_xyz(0.0, pool_y, POOL_Z)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        DespawnOnExit(Sections::Underworld),
    ));

    // NPC at the near pool edge, inverted. Rotates upright to face the player.
    let pool_near_z = POOL_Z + POOL_SIZE * 0.5;
    let npc_scene: Handle<Scene> = asset_server.load(GltfAssetLabel::Scene(0).from_asset(NPC_PATH));
    commands
        .spawn((
            UnderworldNpc,
            SceneRoot(npc_scene),
            Transform::from_xyz(0.0, pool_y, pool_near_z)
                .with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
            DespawnOnExit(Sections::Underworld),
        ))
        .observe(start_npc_torch);
}

fn start_npc_torch(
    trigger: On<SceneInstanceReady>,
    anim: Res<UnderworldNpcAnimation>,
    mut commands: Commands,
    children: Query<&Children>,
    mut players: Query<(Entity, &mut AnimationPlayer)>,
) {
    for child in children.iter_descendants(trigger.entity) {
        if let Ok((anim_entity, mut player)) = players.get_mut(child) {
            player.play(anim.torch).repeat();
            commands
                .entity(anim_entity)
                .insert(AnimationGraphHandle(anim.graph.clone()));
            break;
        }
    }
}

fn exit_underworld(mut commands: Commands) {
    commands.insert_resource(GlobalAmbientLight::NONE);
}

fn underworld_terrain_follow(
    mut player: Query<&mut Transform, With<Player>>,
    noise: Res<TerrainNoise>,
) {
    let Ok(mut transform) = player.single_mut() else {
        return;
    };

    // Clamp to corridor bounds.
    transform.translation.x = transform.translation.x.clamp(
        -(CORRIDOR_HALF_WIDTH - CLAMP_MARGIN),
        CORRIDOR_HALF_WIDTH - CLAMP_MARGIN,
    );
    let pool_edge = POOL_Z + POOL_SIZE * 0.5 + CLAMP_MARGIN;
    transform.translation.z = transform.translation.z.clamp(pool_edge, -WALL_WIDTH);

    // Follow floor height.
    let floor_y = corridor_floor_height(transform.translation.x, transform.translation.z, &noise);
    transform.translation.y = floor_y + EYE_HEIGHT;
}

fn underworld_pool_check(
    player: Query<(&Transform, &PlayerLook), With<Player>>,
    mut state: ResMut<UnderworldState>,
) {
    if !matches!(state.phase, UnderworldPhase::Walking) {
        return;
    }
    let Ok((transform, look)) = player.single() else {
        return;
    };

    let dist_to_pool =
        Vec2::new(transform.translation.x, transform.translation.z - POOL_Z).length();

    if dist_to_pool < POOL_TRIGGER_DIST && look.pitch < POOL_TRIGGER_PITCH {
        state.phase = UnderworldPhase::Rotating;
        state.timer = 0.0;
    }
}

fn underworld_npc_rotate(
    mut npc: Query<&mut Transform, With<UnderworldNpc>>,
    mut state: ResMut<UnderworldState>,
    mut next_state: ResMut<NextState<Sections>>,
    time: Res<Time>,
) {
    match state.phase {
        UnderworldPhase::Rotating => {
            state.timer += time.delta_secs();
            let t = (state.timer / NPC_ROTATION_DURATION).min(1.0);

            if let Ok(mut transform) = npc.single_mut() {
                let angle = std::f32::consts::PI * (1.0 + t);
                transform.rotation = Quat::from_rotation_x(angle);
            }

            if t >= 1.0 {
                state.phase = UnderworldPhase::Waiting;
                state.timer = 0.0;
            }
        }
        UnderworldPhase::Waiting => {
            state.timer += time.delta_secs();
            if state.timer >= NPC_WAIT_DURATION {
                next_state.set(Sections::Stairs);
            }
        }
        UnderworldPhase::Walking => {}
    }
}
