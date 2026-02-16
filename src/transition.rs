// Full-screen title cards that fade in and out between sections.

use bevy::prelude::*;

use crate::sections::Sections;

pub struct TransitionPlugin;

impl Plugin for TransitionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(Sections::Chase),
            |commands: Commands| spawn_card(commands, "I: Dream"),
        )
        .add_systems(
            OnEnter(Sections::Underworld),
            |commands: Commands| spawn_card(commands, "II: Deep"),
        )
        .add_systems(
            OnEnter(Sections::Stairs),
            |commands: Commands| spawn_card(commands, "III: Gradient Ascent"),
        )
        .add_systems(
            OnEnter(Sections::Awaken),
            |commands: Commands| spawn_card(commands, "IV: Awakening"),
        )
        .add_systems(Update, fade_card);
    }
}

const FADE_IN: f32 = 0.1;
const HOLD: f32 = 1.5;
const FADE_OUT: f32 = 1.0;
const TOTAL: f32 = FADE_IN + HOLD + FADE_OUT;

#[derive(Resource)]
struct CardTimer(f32);

#[derive(Component)]
struct CardRoot;

#[derive(Component)]
struct CardText;

fn spawn_card(mut commands: Commands, title: &str) {
    // Despawn any existing card from a previous section.
    commands.insert_resource(CardTimer(0.0));

    commands
        .spawn((
            CardRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::BLACK),
            GlobalZIndex(100),
        ))
        .with_children(|parent| {
            parent.spawn((
                CardText,
                Text::new(title),
                TextFont {
                    font_size: 48.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
            ));
        });
}

fn fade_card(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: Option<ResMut<CardTimer>>,
    roots: Query<Entity, With<CardRoot>>,
    mut texts: Query<&mut TextColor, With<CardText>>,
    mut backgrounds: Query<&mut BackgroundColor, With<CardRoot>>,
) {
    let Some(timer) = timer.as_mut() else {
        return;
    };

    timer.0 += time.delta_secs();
    let t = timer.0;

    if t >= TOTAL {
        // Done — despawn card and remove timer.
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        commands.remove_resource::<CardTimer>();
        return;
    }

    // Compute text and background alpha.
    let text_alpha;
    let bg_alpha;

    if t < FADE_IN {
        // Fade text in, background stays opaque.
        text_alpha = t / FADE_IN;
        bg_alpha = 1.0;
    } else if t < FADE_IN + HOLD {
        // Hold.
        text_alpha = 1.0;
        bg_alpha = 1.0;
    } else {
        // Fade everything out.
        let fade_t = (t - FADE_IN - HOLD) / FADE_OUT;
        text_alpha = 1.0 - fade_t;
        bg_alpha = 1.0 - fade_t;
    }

    for mut color in &mut texts {
        color.0 = Color::srgba(1.0, 1.0, 1.0, text_alpha);
    }
    for mut bg in &mut backgrounds {
        bg.0 = Color::srgba(0.0, 0.0, 0.0, bg_alpha);
    }
}
