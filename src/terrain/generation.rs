/// Noise sampler management for chunk generation
use bevy::prelude::*;
use rand::Rng;

use super::chunk::ChunkEdgeHeights;

/// Axis visible in FOV (< 90 degrees)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum VisibleAxis {
    #[default]
    North,
    East,
    South,
    West,
}

impl From<VisibleAxis> for Dir3 {
    fn from(axis: VisibleAxis) -> Dir3 {
        match axis {
            VisibleAxis::North => Dir3::NEG_Z,
            VisibleAxis::East => Dir3::X,
            VisibleAxis::South => Dir3::Z,
            VisibleAxis::West => Dir3::NEG_X,
        }
    }
}

impl From<VisibleAxis> for Vec3 {
    fn from(axis: VisibleAxis) -> Vec3 {
        let dir: Dir3 = axis.into();
        dir.as_vec3()
    }
}

impl VisibleAxis {
    pub fn left(self) -> VisibleAxis {
        match self {
            VisibleAxis::North => VisibleAxis::West,
            VisibleAxis::East => VisibleAxis::North,
            VisibleAxis::South => VisibleAxis::East,
            VisibleAxis::West => VisibleAxis::South,
        }
    }

    pub fn right(self) -> VisibleAxis {
        match self {
            VisibleAxis::North => VisibleAxis::East,
            VisibleAxis::East => VisibleAxis::South,
            VisibleAxis::South => VisibleAxis::West,
            VisibleAxis::West => VisibleAxis::North,
        }
    }

    /// Unit direction in the XZ plane (x, z).
    pub fn dir_2d(self) -> Vec2 {
        match self {
            VisibleAxis::North => Vec2::new(0.0, -1.0),
            VisibleAxis::East => Vec2::new(1.0, 0.0),
            VisibleAxis::South => Vec2::new(0.0, 1.0),
            VisibleAxis::West => Vec2::new(-1.0, 0.0),
        }
    }

    pub fn left_quadrant(self) -> Quadrant {
        match self {
            VisibleAxis::North => Quadrant::NorthWest,
            VisibleAxis::East => Quadrant::NorthEast,
            VisibleAxis::South => Quadrant::SouthEast,
            VisibleAxis::West => Quadrant::SouthWest,
        }
    }

    pub fn right_quadrant(self) -> Quadrant {
        match self {
            VisibleAxis::North => Quadrant::NorthEast,
            VisibleAxis::East => Quadrant::SouthEast,
            VisibleAxis::South => Quadrant::SouthWest,
            VisibleAxis::West => Quadrant::NorthWest,
        }
    }
}

/// Quadrants visible in FOV (< 90 degrees)
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum Quadrant {
    #[default]
    NorthWest,
    NorthEast,
    SouthEast,
    SouthWest,
}

impl Quadrant {
    pub fn left(self) -> Quadrant {
        match self {
            Quadrant::NorthWest => Quadrant::SouthWest,
            Quadrant::NorthEast => Quadrant::NorthWest,
            Quadrant::SouthEast => Quadrant::NorthEast,
            Quadrant::SouthWest => Quadrant::SouthEast,
        }
    }

    pub fn right(self) -> Quadrant {
        match self {
            Quadrant::NorthWest => Quadrant::NorthEast,
            Quadrant::NorthEast => Quadrant::SouthEast,
            Quadrant::SouthEast => Quadrant::SouthWest,
            Quadrant::SouthWest => Quadrant::NorthWest,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Quadrant::NorthWest => 0,
            Quadrant::NorthEast => 1,
            Quadrant::SouthEast => 2,
            Quadrant::SouthWest => 3,
        }
    }
}

/// Quadrant colour in debug mode
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Reflect)]
pub enum DebugColour {
    #[default]
    Red,
    Green,
    Blue,
    Yellow,
    Cyan,
    Magenta,
    Orange,
    White,
}

impl DebugColour {
    pub const ALL: [DebugColour; 8] = [
        DebugColour::Red,
        DebugColour::Green,
        DebugColour::Blue,
        DebugColour::Yellow,
        DebugColour::Cyan,
        DebugColour::Magenta,
        DebugColour::Orange,
        DebugColour::White,
    ];

    pub fn next(self) -> DebugColour {
        match self {
            DebugColour::Red => DebugColour::Green,
            DebugColour::Green => DebugColour::Blue,
            DebugColour::Blue => DebugColour::Yellow,
            DebugColour::Yellow => DebugColour::Cyan,
            DebugColour::Cyan => DebugColour::Magenta,
            DebugColour::Magenta => DebugColour::Orange,
            DebugColour::Orange => DebugColour::White,
            DebugColour::White => DebugColour::Red,
        }
    }
}

impl From<DebugColour> for Color {
    fn from(_colour: DebugColour) -> Color {
        /*
        match colour {
            DebugColour::Red => Srgba::RED.into(),
            DebugColour::Green => Srgba::GREEN.into(),
            DebugColour::Blue => Srgba::BLUE.into(),
            DebugColour::Yellow => Srgba::new(1.0, 1.0, 0.0, 1.0).into(),
            DebugColour::Cyan => Srgba::new(0.0, 1.0, 1.0, 1.0).into(),
            DebugColour::Magenta => Srgba::new(1.0, 0.0, 1.0, 1.0).into(),
            DebugColour::Orange => Srgba::new(1.0, 0.5, 0.0, 1.0).into(),
            DebugColour::White => Srgba::WHITE.into(),
        }
        */
        Srgba::new(0.1, 0.6, 0.1, 1.0).into()
    }
}

/// Samples noise for two visible quadrants from two planes in noise space.
/// The left quadrant maps through (left_axis, center_axis) and the right
/// through (center_axis, right_axis). The mapping is rotated 90 degrees
/// between them so that center_axis is sampled along the shared seam,
/// giving C0 continuity.
#[derive(Clone, Copy, PartialEq, Debug, Reflect, Resource)]
pub struct NoiseSampler {
    /// World space axis that is currently visible
    pub visible_axis: VisibleAxis,
    /// Left noise space axis, corresponding to the across-axis of the left quadrant
    pub left_axis: Dir3,
    /// Center noise space axis, sampled along the seam between quadrants
    pub center_axis: Dir3,
    /// Right noise space axis, corresponding to the across-axis of the right quadrant
    pub right_axis: Dir3,
    /// Point in noise space corresponding to the quadrant origin
    pub noise_origin: Vec3,
    /// World-space (x, z) origin where the four quadrants meet
    pub quadrant_origin: Vec2,
}

impl Default for NoiseSampler {
    fn default() -> NoiseSampler {
        NoiseSampler {
            visible_axis: VisibleAxis::North,
            left_axis: Dir3::NEG_X,
            center_axis: Dir3::NEG_Z,
            right_axis: Dir3::X,
            noise_origin: Vec3::ZERO,
            quadrant_origin: Vec2::ZERO,
        }
    }
}

impl NoiseSampler {
    /// Map a world-space (x, z) position to a 3D noise-space coordinate.
    /// Points left of the seam sample the left plane; points right sample
    /// the right plane. Both share center_axis along the seam.
    pub fn noise_point(&self, wx: f32, wz: f32, noise_scale: f32) -> Vec3 {
        let d = Vec2::new(wx - self.quadrant_origin.x, wz - self.quadrant_origin.y);
        let visible_2d = self.visible_axis.dir_2d();
        let left_2d = self.visible_axis.left().dir_2d();
        let along = d.dot(visible_2d);
        let lateral = d.dot(left_2d);

        let across_component = if lateral >= 0.0 {
            lateral * noise_scale * *self.left_axis
        } else {
            (-lateral) * noise_scale * *self.right_axis
        };

        self.noise_origin + along * noise_scale * *self.center_axis + across_component
    }

    /// Which named quadrant a world point falls in.
    pub fn quadrant_at(&self, wx: f32, wz: f32) -> Quadrant {
        let north = wz < self.quadrant_origin.y;
        let east = wx >= self.quadrant_origin.x;
        match (north, east) {
            (true, false) => Quadrant::NorthWest,
            (true, true) => Quadrant::NorthEast,
            (false, true) => Quadrant::SouthEast,
            (false, false) => Quadrant::SouthWest,
        }
    }

    /// Snap the origin to the chunk boundary just behind the player along the
    /// visible axis. Adjusts noise_origin to compensate, preserving all terrain heights.
    pub fn slide_origin(&mut self, player_pos: Vec2, chunk_size: f32, noise_scale: f32) {
        let visible_2d = self.visible_axis.dir_2d();
        let p_along = player_pos.dot(visible_2d);
        let snapped = (p_along / chunk_size).floor() * chunk_size;
        let old_along = self.quadrant_origin.dot(visible_2d);
        let d_along = snapped - old_along;
        self.noise_origin += d_along * noise_scale * *self.center_axis;
        self.quadrant_origin += visible_2d * d_along;
    }

    /// Rotate the noise sampler 90 degrees left. The old left quadrant
    /// survives as the new right; the new left gets fresh noise.
    pub fn rotate_left(self, player_pos: Vec2, chunk_size: f32, noise_scale: f32) -> NoiseSampler {
        let new_visible = self.visible_axis.left();
        let new_visible_2d = new_visible.dir_2d();
        let snapped_along = (player_pos.dot(new_visible_2d) / chunk_size).floor() * chunk_size;
        let cross_2d = new_visible.left().dir_2d();
        let new_origin =
            new_visible_2d * snapped_along + cross_2d * self.quadrant_origin.dot(cross_2d);

        let new_left = random_orthogonal_dir3(self.left_axis);
        let new_center = self.left_axis;
        let new_right = self.center_axis;

        // Adjust noise_origin to preserve the surviving quadrant (old left → new right).
        let d = new_origin - self.quadrant_origin;
        let d_along = d.dot(new_visible_2d);
        let d_across = -d.dot(new_visible.left().dir_2d());
        let new_noise_origin = self.noise_origin
            + d_along * noise_scale * *new_center
            + d_across * noise_scale * *new_right;

        NoiseSampler {
            visible_axis: new_visible,
            left_axis: new_left,
            center_axis: new_center,
            right_axis: new_right,
            noise_origin: new_noise_origin,
            quadrant_origin: new_origin,
        }
    }

    /// Rotate the noise sampler 90 degrees right. The old right quadrant
    /// survives as the new left; the new right gets fresh noise.
    pub fn rotate_right(self, player_pos: Vec2, chunk_size: f32, noise_scale: f32) -> NoiseSampler {
        let new_visible = self.visible_axis.right();
        let new_visible_2d = new_visible.dir_2d();
        let snapped_along = (player_pos.dot(new_visible_2d) / chunk_size).floor() * chunk_size;
        let cross_2d = new_visible.left().dir_2d();
        let new_origin =
            new_visible_2d * snapped_along + cross_2d * self.quadrant_origin.dot(cross_2d);

        let new_left = self.center_axis;
        let new_center = self.right_axis;
        let new_right = random_orthogonal_dir3(self.right_axis);

        // Adjust noise_origin to preserve the surviving quadrant (old right → new left).
        let d = new_origin - self.quadrant_origin;
        let new_left_2d = new_visible.left().dir_2d();
        let d_along = d.dot(new_visible_2d);
        let d_across = d.dot(new_left_2d);
        let new_noise_origin = self.noise_origin
            + d_across * noise_scale * *new_left
            + d_along * noise_scale * *new_center;

        NoiseSampler {
            visible_axis: new_visible,
            left_axis: new_left,
            center_axis: new_center,
            right_axis: new_right,
            noise_origin: new_noise_origin,
            quadrant_origin: new_origin,
        }
    }
}

/// A chunk whose mesh was generated with a now-stale NoiseSampler.
/// Adjacent chunks blend heights to avoid visible seams at the boundary.
/// Stores actual edge vertex heights so boundary vertices match exactly.
#[derive(Clone, Copy, Debug)]
pub struct StaleRegion {
    pub sampler: NoiseSampler,
    pub grid_pos: (i32, i32),
    pub edge_heights: ChunkEdgeHeights,
}

/// Blend factor based on distance from a stale chunk boundary.
/// Returns 0.0 at the stale chunk edge, 1.0 beyond one chunk_size away.
pub fn blend_factor(wx: f32, wz: f32, stale: &StaleRegion, chunk_size: f32) -> f32 {
    let min_x = stale.grid_pos.0 as f32 * chunk_size;
    let max_x = min_x + chunk_size;
    let min_z = stale.grid_pos.1 as f32 * chunk_size;
    let max_z = min_z + chunk_size;

    let dx = f32::max(0.0, f32::max(min_x - wx, wx - max_x));
    let dz = f32::max(0.0, f32::max(min_z - wz, wz - max_z));
    let dist = (dx * dx + dz * dz).sqrt();

    smoothstep(0.0, chunk_size, dist)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Select random Vec3 on unit sphere
fn random_unit_vec3() -> Vec3 {
    let mut rng = rand::rng();
    loop {
        let v = Vec3::new(
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
            rng.random_range(-1.0..1.0),
        );
        if v.length_squared() > 0.01 {
            return v.normalize();
        }
    }
}

/// Select random Dir3 orthogonal to that passed in
fn random_orthogonal_dir3(dir: Dir3) -> Dir3 {
    loop {
        let v = random_unit_vec3();
        let projected = v - v.dot(*dir) * *dir;
        if projected.length_squared() > 0.01 {
            return Dir3::new(projected.normalize())
                .expect("Vec3 should always be valid direction");
        }
    }
}
