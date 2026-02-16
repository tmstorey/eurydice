// Chase section
use bevy::prelude::*;

use crate::dream::DreamSettings;
use crate::npc::{Npc, NpcChevron};
use crate::player::Player;
use crate::sections::{PlotFlags, Sections};
use crate::terrain::{RotationCount, SpawnedChunks, TerrainChunk};

pub struct ChasePlugin;

impl Plugin for ChasePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Sections::Chase), reset_chase_state)
            .add_systems(
                Update,
                (chase_dream_ramp, chase_chevron_degrade, chase_npc_vanish)
                    .chain()
                    .run_if(in_state(Sections::Chase)),
            )
            .add_systems(OnExit(Sections::Chase), exit_chase);
    }
}

fn reset_chase_state(mut plot_flags: ResMut<PlotFlags>, mut rotation_count: ResMut<RotationCount>) {
    *plot_flags = PlotFlags::default();
    rotation_count.0 = 0;
}

/// Base dream intensity increase per second.
const DREAM_BASE_RATE: f32 = 0.005;
/// Multiplier when the NPC chevron is visible (NPC is far away).
const DREAM_CHEVRON_MULTIPLIER: f32 = 2.0;
/// Flat intensity bump per terrain rotation.
const DREAM_ROTATION_BUMP: f32 = 0.03;
/// Dream intensity at which the chevron turns red and NPC can vanish.
const CHEVRON_RED_THRESHOLD: f32 = 0.7;
/// Max chevron shake offset in pixels at full intensity.
const CHEVRON_MAX_SHAKE: f32 = 8.0;

fn chase_dream_ramp(
    mut dream_query: Query<&mut DreamSettings>,
    chevron_query: Query<&Visibility, With<NpcChevron>>,
    mut rotation_count: ResMut<RotationCount>,
    time: Res<Time>,
) {
    let Ok(mut settings) = dream_query.single_mut() else {
        return;
    };

    let dt = time.delta_secs();
    let mut rate = DREAM_BASE_RATE;

    // Faster when the chevron is visible (NPC is far enough to show it).
    if let Ok(visibility) = chevron_query.single() {
        if *visibility != Visibility::Hidden {
            rate *= DREAM_CHEVRON_MULTIPLIER;
        }
    }

    settings.intensity += rate * dt;

    // Flat bump per terrain rotation.
    let rotations = rotation_count.0;
    if rotations > 0 {
        settings.intensity += DREAM_ROTATION_BUMP * rotations as f32;
        rotation_count.0 = 0;
    }

    settings.intensity = settings.intensity.min(1.0);
}

fn chase_chevron_degrade(
    mut chevron_query: Query<(&mut Node, &mut TextColor, &Visibility), With<NpcChevron>>,
    dream_query: Query<&DreamSettings>,
) {
    let Ok(settings) = dream_query.single() else {
        return;
    };
    let Ok((mut node, mut color, visibility)) = chevron_query.single_mut() else {
        return;
    };

    // Only shake when chevron is visible.
    if *visibility == Visibility::Hidden {
        return;
    }

    // Apply random shake proportional to intensity.
    if settings.intensity > 0.1 {
        let shake = settings.intensity * CHEVRON_MAX_SHAKE;
        // Use time-based pseudo-random offset (changes every frame).
        let t = settings.time * 60.0;
        let offset_x = (t.sin() * 1.7 + (t * 2.3).cos()) * shake;
        let offset_y = ((t * 1.3).cos() + (t * 3.1).sin()) * shake;

        // Offset the existing position.
        if let Val::Px(ref mut left) = node.left {
            *left += offset_x;
        }
        if let Val::Px(ref mut top) = node.top {
            *top += offset_y;
        }
    }

    // Turn red above threshold.
    if settings.intensity >= CHEVRON_RED_THRESHOLD {
        color.0 = Color::linear_rgb(1.0, 0.0, 0.0);
    }
}

fn chase_npc_vanish(
    mut commands: Commands,
    npc_query: Query<(Entity, &GlobalTransform), With<Npc>>,
    camera_query: Query<&GlobalTransform, With<Player>>,
    dream_query: Query<&DreamSettings>,
    mut next_state: ResMut<NextState<Sections>>,
) {
    let Ok(settings) = dream_query.single() else {
        return;
    };
    if settings.intensity < CHEVRON_RED_THRESHOLD {
        return;
    };
    if settings.intensity >= 1.0 {
        next_state.set(Sections::Underworld);
    }

    let Ok((npc_entity, npc_global)) = npc_query.single() else {
        return;
    };
    let Ok(camera_global) = camera_query.single() else {
        return;
    };

    // Check if NPC is behind the camera.
    let npc_world = npc_global.translation();
    let view_matrix = camera_global.affine().inverse();
    let npc_view = view_matrix.transform_point3(npc_world);

    // In Bevy's view space, camera looks down -Z, so npc_view.z >= 0 means behind.
    if npc_view.z >= 0.0 {
        commands.entity(npc_entity).despawn();
        next_state.set(Sections::Underworld);
    }
}

fn exit_chase(
    mut commands: Commands,
    chunks: Query<Entity, With<TerrainChunk>>,
    npc: Query<Entity, With<Npc>>,
    lights: Query<Entity, With<DirectionalLight>>,
    mut chevron: Query<&mut Visibility, With<NpcChevron>>,
    mut dream: Query<&mut DreamSettings>,
    mut spawned: ResMut<SpawnedChunks>,
) {
    for entity in &chunks {
        commands.entity(entity).despawn();
    }
    spawned.0.clear();

    if let Ok(entity) = npc.single() {
        commands.entity(entity).despawn();
    }

    for entity in &lights {
        commands.entity(entity).despawn();
    }

    if let Ok(mut vis) = chevron.single_mut() {
        *vis = Visibility::Hidden;
    }

    if let Ok(mut settings) = dream.single_mut() {
        settings.intensity = 0.0;
    }
}
