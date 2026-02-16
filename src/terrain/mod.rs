// Terrain generation and chunk management.
mod chunk;
pub(crate) mod generation;
mod objects;

use bevy::prelude::*;
use noiz::prelude::{common_noise::*, *};
use std::collections::HashSet;

use crate::player::Player;
use crate::sections::Sections;
use chunk::{ChunkEdgeHeights, generate_chunk_mesh};

pub use chunk::terrain_height;
use generation::{DebugColour, NoiseSampler, StaleRegion, VisibleAxis};
use objects::{BlueNoisePoints, TerrainObjectAssets};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainNoise>()
            .init_resource::<NoiseSampler>()
            .insert_resource(TerrainConfig::default())
            .insert_resource(SpawnedChunks::default())
            .init_resource::<ChunkColours>()
            .init_resource::<StaleChunk>()
            .init_resource::<RotationCount>()
            .add_systems(
                Startup,
                (
                    setup_terrain_material,
                    objects::setup_blue_noise,
                    objects::load_terrain_objects,
                ),
            )
            .add_systems(
                Update,
                (
                    detect_rotation,
                    update_origin,
                    manage_chunks,
                    follow_terrain_height,
                )
                    .chain()
                    .run_if(in_state(Sections::Chase)),
            );
    }
}

#[derive(Resource)]
pub struct TerrainNoise(pub Noise<Fbm<Perlin>>);

impl Default for TerrainNoise {
    fn default() -> TerrainNoise {
        let mut noise: Noise<Fbm<Perlin>> = Noise::<Fbm<Perlin>>::default();
        noise.set_seed(42);
        noise.set_frequency(2.0);
        TerrainNoise(noise)
    }
}

#[derive(Resource)]
pub struct TerrainConfig {
    pub chunk_size: f32,
    pub chunk_resolution: usize,
    pub amplitude: f32,
    pub noise_scale: f32,
    pub render_radius: i32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            chunk_size: 8.0,
            chunk_resolution: 5,
            amplitude: 8.0,
            noise_scale: 0.01,
            render_radius: 16,
        }
    }
}

#[derive(Resource)]
struct TerrainMaterials {
    by_colour: [Handle<StandardMaterial>; 8],
}

#[derive(Resource, Default)]
pub struct SpawnedChunks(pub HashSet<(i32, i32)>);

#[derive(Resource)]
struct ChunkColours {
    quadrant_colours: [DebugColour; 4],
    next_colour: DebugColour,
}

impl Default for ChunkColours {
    fn default() -> Self {
        Self {
            quadrant_colours: [
                DebugColour::Red,   // NW (initial left)
                DebugColour::Green, // NE (initial right)
                DebugColour::Red,   // SE
                DebugColour::Red,   // SW
            ],
            next_colour: DebugColour::Blue,
        }
    }
}

#[derive(Resource, Default)]
pub struct StaleChunk(pub Option<StaleRegion>);

/// Counts terrain rotations so other systems can react to them.
#[derive(Resource, Default)]
pub struct RotationCount(pub u32);

#[derive(Component)]
pub struct TerrainChunk {
    pub grid_pos: (i32, i32),
}

const EYE_HEIGHT: f32 = 1.5;
/// Max chunks to generate per frame to avoid hitches.
const MAX_SPAWNS_PER_FRAME: usize = 64;

fn setup_terrain_material(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let by_colour = DebugColour::ALL.map(|colour| {
        let base: Color = colour.into();
        materials.add(StandardMaterial {
            base_color: base,
            perceptual_roughness: 0.9,
            ..default()
        })
    });
    commands.insert_resource(TerrainMaterials { by_colour });
}

/// Detect when the player crosses a 45-degree sector boundary and
/// rotate the noise sampler, despawning the retired quadrant.
fn detect_rotation(
    mut commands: Commands,
    mut sampler: ResMut<NoiseSampler>,
    mut spawned: ResMut<SpawnedChunks>,
    mut colours: ResMut<ChunkColours>,
    mut stale: ResMut<StaleChunk>,
    mut rotation_count: ResMut<RotationCount>,
    config: Res<TerrainConfig>,
    player: Query<&Transform, With<Player>>,
    chunks: Query<(Entity, &TerrainChunk, Option<&ChunkEdgeHeights>)>,
) {
    let Ok(transform) = player.single() else {
        return;
    };
    let forward = *transform.forward();

    let sector = if forward.z.abs() >= forward.x.abs() {
        if forward.z < 0.0 {
            VisibleAxis::North
        } else {
            VisibleAxis::South
        }
    } else if forward.x > 0.0 {
        VisibleAxis::East
    } else {
        VisibleAxis::West
    };

    if sector == sampler.visible_axis {
        return;
    }

    let player_pos = Vec2::new(transform.translation.x, transform.translation.z);
    let player_grid = (
        (player_pos.x / config.chunk_size).floor() as i32,
        (player_pos.y / config.chunk_size).floor() as i32,
    );

    // Determine which quadrant is being retired and whether the player
    // chunk sits in it. If so, record the current sampler so adjacent
    // chunks can blend toward the stale mesh.
    let rotating_right = sector == sampler.visible_axis.right();
    let retiring = if rotating_right {
        sampler.visible_axis.left_quadrant()
    } else {
        sampler.visible_axis.right_quadrant()
    };
    let player_center = Vec2::new(
        (player_grid.0 as f32 + 0.5) * config.chunk_size,
        (player_grid.1 as f32 + 0.5) * config.chunk_size,
    );
    let player_quadrant = sampler.quadrant_at(player_center.x, player_center.y);

    if player_quadrant == retiring {
        // Only update stale if it's empty or tracking a different chunk.
        // When the player hasn't moved, the mesh is still from the
        // originally-recorded sampler, so we keep that one.
        if stale.0.as_ref().is_none_or(|s| s.grid_pos != player_grid) {
            let player_edges = chunks
                .iter()
                .find(|(_, chunk, _)| chunk.grid_pos == player_grid)
                .and_then(|(_, _, edges)| edges.copied());

            if let Some(edge_heights) = player_edges {
                stale.0 = Some(StaleRegion {
                    sampler: *sampler,
                    grid_pos: player_grid,
                    edge_heights,
                });
            }
        }
    }

    let (new_sampler, fresh) = if rotating_right {
        let new = sampler.rotate_right(player_pos, config.chunk_size, config.noise_scale);
        (new, sector.right_quadrant())
    } else {
        let new = sampler.rotate_left(player_pos, config.chunk_size, config.noise_scale);
        (new, sector.left_quadrant())
    };

    // Despawn everything behind the new origin along the new visible axis.
    let new_visible_2d = sector.dir_2d();
    let origin_along = new_sampler.quadrant_origin.dot(new_visible_2d);

    for (entity, chunk, _) in &chunks {
        if chunk.grid_pos == player_grid {
            continue;
        }
        let center_x = (chunk.grid_pos.0 as f32 + 0.5) * config.chunk_size;
        let center_z = (chunk.grid_pos.1 as f32 + 0.5) * config.chunk_size;
        if Vec2::new(center_x, center_z).dot(new_visible_2d) < origin_along {
            if stale
                .0
                .as_ref()
                .is_some_and(|s| s.grid_pos == chunk.grid_pos)
            {
                stale.0 = None;
            }
            commands.entity(entity).despawn();
            spawned.0.remove(&chunk.grid_pos);
        }
    }

    *sampler = new_sampler;
    colours.quadrant_colours[fresh.index()] = colours.next_colour;
    colours.next_colour = colours.next_colour.next();
    rotation_count.0 += 1;
}

/// Keep the quadrant origin one chunk behind the player along the visible axis.
fn update_origin(
    mut sampler: ResMut<NoiseSampler>,
    config: Res<TerrainConfig>,
    player: Query<&Transform, With<Player>>,
) {
    let Ok(transform) = player.single() else {
        return;
    };
    let player_pos = Vec2::new(transform.translation.x, transform.translation.z);
    sampler.slide_origin(player_pos, config.chunk_size, config.noise_scale);
}

/// Spawn and despawn terrain chunks based on distance and visibility.
fn manage_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<TerrainMaterials>,
    noise: Res<TerrainNoise>,
    config: Res<TerrainConfig>,
    sampler: Res<NoiseSampler>,
    colours: Res<ChunkColours>,
    mut stale: ResMut<StaleChunk>,
    mut spawned: ResMut<SpawnedChunks>,
    blue_noise: Res<BlueNoisePoints>,
    object_assets: Res<TerrainObjectAssets>,
    player: Query<&Transform, With<Player>>,
    chunks: Query<(Entity, &TerrainChunk)>,
) {
    let Ok(transform) = player.single() else {
        return;
    };
    let player_pos = transform.translation;

    let player_cx = (player_pos.x / config.chunk_size).floor() as i32;
    let player_cz = (player_pos.z / config.chunk_size).floor() as i32;
    let radius = config.render_radius;
    let radius_sq = radius * radius;

    let visible_2d = sampler.visible_axis.dir_2d();
    let player_center = Vec2::new(
        (player_cx as f32 + 0.5) * config.chunk_size,
        (player_cz as f32 + 0.5) * config.chunk_size,
    );
    let player_along = player_center.dot(visible_2d);

    // Despawn chunks that are too far or behind the player on the visible axis.
    for (entity, chunk) in &chunks {
        let dx = chunk.grid_pos.0 - player_cx;
        let dz = chunk.grid_pos.1 - player_cz;
        let dist_sq = dx * dx + dz * dz;
        let too_far = dist_sq > (radius + 2) * (radius + 2);

        let center = Vec2::new(
            (chunk.grid_pos.0 as f32 + 0.5) * config.chunk_size,
            (chunk.grid_pos.1 as f32 + 0.5) * config.chunk_size,
        );
        let behind = center.dot(visible_2d) < player_along;

        if too_far || behind {
            if stale
                .0
                .as_ref()
                .is_some_and(|s| s.grid_pos == chunk.grid_pos)
            {
                stale.0 = None;
            }
            commands.entity(entity).despawn();
            spawned.0.remove(&chunk.grid_pos);
        }
    }

    // Spawn missing chunks forward of the player on the visible axis.
    let stale_ref = stale.0.as_ref();
    let mut spawned_this_frame = 0;
    for cz in (player_cz - radius)..(player_cz + radius) {
        for cx in (player_cx - radius)..(player_cx + radius) {
            if spawned_this_frame >= MAX_SPAWNS_PER_FRAME {
                return;
            }
            if spawned.0.contains(&(cx, cz)) {
                continue;
            }

            let dx = cx - player_cx;
            let dz = cz - player_cz;
            if dx * dx + dz * dz > radius_sq {
                continue;
            }

            let center = Vec2::new(
                (cx as f32 + 0.5) * config.chunk_size,
                (cz as f32 + 0.5) * config.chunk_size,
            );
            if center.dot(visible_2d) < player_along {
                continue;
            }

            let quadrant = sampler.quadrant_at(center.x, center.y);
            let colour = colours.quadrant_colours[quadrant.index()];
            let (mesh, edge_heights) =
                generate_chunk_mesh(cx, cz, &config, &noise, &sampler, stale_ref);
            let mesh_handle = meshes.add(mesh);

            commands
                .spawn((
                    TerrainChunk { grid_pos: (cx, cz) },
                    edge_heights,
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(materials.by_colour[colour as usize].clone()),
                ))
                .with_children(|parent| {
                    objects::spawn_chunk_objects(
                        parent,
                        cx,
                        cz,
                        &config,
                        &noise,
                        &sampler,
                        stale_ref,
                        &blue_noise,
                        &object_assets,
                    );
                });

            spawned.0.insert((cx, cz));
            spawned_this_frame += 1;
        }
    }
}

/// Sample terrain height at the player position so they follow the ground.
/// Uses blended height when a stale chunk is active to match the actual mesh.
fn follow_terrain_height(
    mut player: Query<&mut Transform, With<Player>>,
    noise: Res<TerrainNoise>,
    config: Res<TerrainConfig>,
    sampler: Res<NoiseSampler>,
    stale: Res<StaleChunk>,
) {
    let Ok(mut transform) = player.single_mut() else {
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
    transform.translation.y = height + EYE_HEIGHT;
}
