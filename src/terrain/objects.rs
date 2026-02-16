// Terrain object placement using blue noise distribution.
use bevy::prelude::*;
use fast_poisson::Poisson2D;

use super::{TerrainConfig, TerrainNoise};
use crate::terrain::chunk::terrain_height;
use crate::terrain::generation::{NoiseSampler, StaleRegion};

/// Pre-generated blue noise point set for object placement within a chunk.
#[derive(Resource)]
pub struct BlueNoisePoints(Vec<[f32; 2]>);

/// Preloaded scene handles for terrain objects, grouped by category.
#[derive(Resource)]
pub struct TerrainObjectAssets {
    trees: Vec<Handle<Scene>>,
    dead_trees: Vec<Handle<Scene>>,
    rocks: Vec<Handle<Scene>>,
    ground_cover: Vec<Handle<Scene>>,
}

pub fn setup_blue_noise(mut commands: Commands) {
    let points: Vec<[f32; 2]> = Poisson2D::new()
        .with_dimensions([1.0, 1.0], 0.15)
        .with_seed(42)
        .generate();
    commands.insert_resource(BlueNoisePoints(points));
}

pub fn load_terrain_objects(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |name: &str| -> Handle<Scene> {
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(format!("terrain/{name}.gltf")))
    };

    let trees = vec![
        load("Pine_1"),
        load("Pine_2"),
        load("Pine_3"),
        load("Pine_4"),
        load("Pine_5"),
        load("CommonTree_1"),
        load("CommonTree_2"),
        load("CommonTree_3"),
        load("CommonTree_4"),
        load("CommonTree_5"),
    ];

    let dead_trees = vec![
        load("DeadTree_1"),
        load("DeadTree_2"),
        load("DeadTree_3"),
        load("DeadTree_4"),
        load("DeadTree_5"),
    ];

    let rocks = vec![
        load("Rock_Medium_1"),
        load("Rock_Medium_2"),
        load("Rock_Medium_3"),
    ];

    let ground_cover = vec![
        load("Grass_Wispy_Short"),
        load("Grass_Wispy_Tall"),
        load("Grass_Common_Short"),
        load("Grass_Common_Tall"),
        load("Flower_3_Single"),
        load("Flower_3_Group"),
        load("Flower_4_Single"),
        load("Flower_4_Group"),
        load("Mushroom_Common"),
        load("Mushroom_Laetiporus"),
        load("Fern_1"),
        load("Plant_1"),
        load("Plant_1_Big"),
        load("Plant_7"),
        load("Plant_7_Big"),
        load("Clover_1"),
        load("Clover_2"),
        load("Bush_Common"),
        load("Bush_Common_Flowers"),
        load("Pebble_Round_1"),
        load("Pebble_Round_2"),
        load("Pebble_Round_3"),
        load("Pebble_Round_4"),
        load("Pebble_Round_5"),
        load("Pebble_Square_1"),
        load("Pebble_Square_2"),
        load("Pebble_Square_3"),
        load("Pebble_Square_4"),
        load("Pebble_Square_5"),
        load("Pebble_Square_6"),
    ];

    commands.insert_resource(TerrainObjectAssets {
        trees,
        dead_trees,
        rocks,
        ground_cover,
    });
}

/// Spawn terrain objects as children of a chunk entity.
pub fn spawn_chunk_objects(
    parent: &mut ChildSpawnerCommands,
    chunk_x: i32,
    chunk_z: i32,
    config: &TerrainConfig,
    noise: &TerrainNoise,
    sampler: &NoiseSampler,
    stale: Option<&StaleRegion>,
    points: &BlueNoisePoints,
    assets: &TerrainObjectAssets,
) {
    let size = config.chunk_size;
    let origin_x = chunk_x as f32 * size;
    let origin_z = chunk_z as f32 * size;

    for point in &points.0 {
        let wx = origin_x + point[0] * size;
        let wz = origin_z + point[1] * size;

        // Hash the noise-space coordinate for uniform, spatially-independent
        // selection. Using noise_point means the hash changes when the sampler
        // rotates, so objects change with the terrain.
        let p = sampler.noise_point(wx, wz, config.noise_scale);
        let t = hash_vec3(p);

        let scene = if t > 0.998 && t < 1.0 {
            pick(&assets.dead_trees, hash_vec3(p + Vec3::X))
        } else if t > 0.995 {
            pick(&assets.rocks, hash_vec3(p + Vec3::Y))
        } else if t > 0.985 {
            pick(&assets.trees, hash_vec3(p + Vec3::X))
        } else if t > 0.93 {
            pick(&assets.ground_cover, hash_vec3(p + Vec3::Z))
        } else {
            continue;
        };

        let height = terrain_height(
            wx,
            wz,
            noise,
            sampler,
            config.amplitude,
            config.noise_scale,
            size,
            stale,
        );

        parent.spawn((
            SceneRoot(scene.clone()),
            Transform::from_xyz(wx, height, wz),
        ));
    }
}

/// Select an item from a list using a fractional index in [0, 1).
fn pick(items: &[Handle<Scene>], frac: f32) -> &Handle<Scene> {
    let idx = (frac * items.len() as f32) as usize;
    &items[idx.min(items.len() - 1)]
}

/// GPU-style hash producing a uniform value in [0, 1) from a 3D point.
fn hash_vec3(p: Vec3) -> f32 {
    p.dot(Vec3::new(127.1, 311.7, 74.7))
        .sin()
        .mul_add(43758.545, 0.0)
        .fract()
        .abs()
}
