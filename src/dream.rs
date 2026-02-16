// DeepDream style post-processing effect with yellow tint, procedural eyes, swirl tendrils,
// and chromatic aberration.
use bevy::{
    core_pipeline::{
        core_3d::graph::Node3d,
        fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin},
    },
    prelude::*,
    render::{
        extract_component::ExtractComponent,
        render_graph::{InternedRenderLabel, RenderLabel},
        render_resource::ShaderType,
    },
    shader::ShaderRef,
};

pub struct DreamPlugin;

impl Plugin for DreamPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<DreamSettings>::default())
            .add_systems(Update, update_dream_time);

        #[cfg(debug_assertions)]
        app.add_systems(Startup, spawn_intensity_display)
            .add_systems(Update, adjust_intensity);
    }
}

/// Controls the DeepDream post-processing effect. Add to a camera entity.
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
pub struct DreamSettings {
    /// Effect strength from 0.0 (off) to 1.0 (full).
    pub intensity: f32,
    /// Elapsed time in seconds, drives subtle animation.
    pub time: f32,
    pub _align: f32,
    pub _align2: f32,
}

impl FullscreenMaterial for DreamSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/dream.wgsl".into()
    }

    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

fn update_dream_time(mut query: Query<&mut DreamSettings>, time: Res<Time>) {
    for mut settings in &mut query {
        settings.time = time.elapsed_secs();
    }
}

#[cfg(debug_assertions)]
const INTENSITY_STEP: f32 = 0.05;

#[cfg(debug_assertions)]
#[derive(Component)]
struct IntensityDisplay;

#[cfg(debug_assertions)]
fn spawn_intensity_display(mut commands: Commands) {
    commands.spawn((
        IntensityDisplay,
        Text::new("Intensity: 0.00"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

#[cfg(debug_assertions)]
fn adjust_intensity(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dream_query: Query<&mut DreamSettings>,
    mut text_query: Query<&mut Text, With<IntensityDisplay>>,
) {
    let Ok(mut settings) = dream_query.single_mut() else {
        return;
    };

    let mut changed = false;
    if keyboard.just_pressed(KeyCode::ArrowUp) {
        settings.intensity = (settings.intensity + INTENSITY_STEP).min(1.0);
        changed = true;
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) {
        settings.intensity = (settings.intensity - INTENSITY_STEP).max(0.0);
        changed = true;
    }

    if changed {
        if let Ok(mut text) = text_query.single_mut() {
            **text = format!("Intensity: {:.2}", settings.intensity);
        }
    }
}
