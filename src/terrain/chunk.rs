// Terrain chunk mesh generation from 3D noise sampling.
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use noiz::prelude::*;

use super::{TerrainConfig, TerrainNoise};
use crate::terrain::generation::{blend_factor, NoiseSampler, StaleRegion};

/// Actual vertex heights along each edge of a generated chunk mesh.
/// Used to enforce exact height matching at boundaries with stale chunks.
#[derive(Component, Clone, Copy, Debug)]
pub struct ChunkEdgeHeights {
    /// Heights along zi=0 (min z), indexed by xi.
    pub north: [f32; 5],
    /// Heights along zi=res-1 (max z), indexed by xi.
    pub south: [f32; 5],
    /// Heights along xi=0 (min x), indexed by zi.
    pub west: [f32; 5],
    /// Heights along xi=res-1 (max x), indexed by zi.
    pub east: [f32; 5],
}

impl ChunkEdgeHeights {
    /// If vertex (xi, zi) of chunk at (chunk_x, chunk_z) shares a boundary
    /// with the stale chunk at (stale_x, stale_z), return the stored height.
    pub fn shared_height(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        xi: usize,
        zi: usize,
        stale_x: i32,
        stale_z: i32,
        res: usize,
    ) -> Option<f32> {
        let dx = chunk_x - stale_x;
        let dz = chunk_z - stale_z;
        let last = res - 1;

        match (dx, dz) {
            // Directly east of stale: our west edge (xi=0) = stale's east edge
            (1, 0) if xi == 0 => Some(self.east[zi]),
            // Directly west: our east edge (xi=last) = stale's west edge
            (-1, 0) if xi == last => Some(self.west[zi]),
            // Directly south: our north edge (zi=0) = stale's south edge
            (0, 1) if zi == 0 => Some(self.south[xi]),
            // Directly north: our south edge (zi=last) = stale's north edge
            (0, -1) if zi == last => Some(self.north[xi]),
            // Diagonal SE: our NW corner = stale's SE corner
            (1, 1) if xi == 0 && zi == 0 => Some(self.south[last]),
            // Diagonal SW: our NE corner = stale's SW corner
            (-1, 1) if xi == last && zi == 0 => Some(self.south[0]),
            // Diagonal NE: our SW corner = stale's NE corner
            (1, -1) if xi == 0 && zi == last => Some(self.north[last]),
            // Diagonal NW: our SE corner = stale's NW corner
            (-1, -1) if xi == last && zi == last => Some(self.north[0]),
            _ => None,
        }
    }
}

/// Sample terrain height at a world-space position, blending with stale noise if active.
pub fn terrain_height(
    wx: f32,
    wz: f32,
    noise: &TerrainNoise,
    sampler: &NoiseSampler,
    amplitude: f32,
    noise_scale: f32,
    chunk_size: f32,
    stale: Option<&StaleRegion>,
) -> f32 {
    let p = sampler.noise_point(wx, wz, noise_scale);
    let h = noise.0.sample_for::<f32>(p) * amplitude;

    if let Some(stale) = stale {
        let t = blend_factor(wx, wz, stale, chunk_size);
        if t < 1.0 {
            let old_p = stale.sampler.noise_point(wx, wz, noise_scale);
            let old_h = noise.0.sample_for::<f32>(old_p) * amplitude;
            return old_h + t * (h - old_h);
        }
    }
    h
}

/// Generate a terrain mesh for a single chunk at the given grid position.
/// When a stale region is present, heights near its boundary are blended
/// between the old and current noise so the stale chunk's edges match.
pub fn generate_chunk_mesh(
    chunk_x: i32,
    chunk_z: i32,
    config: &TerrainConfig,
    noise: &TerrainNoise,
    sampler: &NoiseSampler,
    stale: Option<&StaleRegion>,
) -> (Mesh, ChunkEdgeHeights) {
    let size = config.chunk_size;
    let res = config.chunk_resolution;
    let step = size / (res - 1) as f32;
    let amplitude = config.amplitude;
    let scale = config.noise_scale;

    let origin_x = chunk_x as f32 * size;
    let origin_z = chunk_z as f32 * size;

    let height_at = |wx: f32, wz: f32| -> f32 {
        terrain_height(wx, wz, noise, sampler, amplitude, scale, size, stale)
    };

    let mut positions = Vec::with_capacity(res * res);
    let mut normals = Vec::with_capacity(res * res);
    let mut indices = Vec::new();

    for zi in 0..res {
        for xi in 0..res {
            let wx = origin_x + xi as f32 * step;
            let wz = origin_z + zi as f32 * step;
            let height = stale
                .and_then(|s| {
                    s.edge_heights.shared_height(
                        chunk_x, chunk_z, xi, zi,
                        s.grid_pos.0, s.grid_pos.1, res,
                    )
                })
                .unwrap_or_else(|| height_at(wx, wz));
            positions.push([wx, height, wz]);

            // Normal from height gradient via central differences.
            let eps = step * 0.5;
            let normal = Vec3::new(
                height_at(wx - eps, wz) - height_at(wx + eps, wz),
                2.0 * eps,
                height_at(wx, wz - eps) - height_at(wx, wz + eps),
            )
            .normalize();
            normals.push(normal.to_array());
        }
    }

    for zi in 0..(res - 1) {
        for xi in 0..(res - 1) {
            let i = (zi * res + xi) as u32;
            let w = res as u32;
            indices.push(i);
            indices.push(i + w);
            indices.push(i + 1);
            indices.push(i + 1);
            indices.push(i + w);
            indices.push(i + w + 1);
        }
    }

    let mut edge_heights = ChunkEdgeHeights {
        north: [0.0; 5],
        south: [0.0; 5],
        west: [0.0; 5],
        east: [0.0; 5],
    };
    for xi in 0..res {
        edge_heights.north[xi] = positions[xi][1];
        edge_heights.south[xi] = positions[(res - 1) * res + xi][1];
    }
    for zi in 0..res {
        edge_heights.west[zi] = positions[zi * res][1];
        edge_heights.east[zi] = positions[zi * res + (res - 1)][1];
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    (mesh, edge_heights)
}
