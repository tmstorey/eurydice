// Main menu

use bevy::prelude::*;

use crate::sections::Sections;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Sections::Menu), setup_menu)
            .add_systems(
                Update,
                (button_visuals, button_actions, credits_back).run_if(in_state(Sections::Menu)),
            );
    }
}

const NORMAL_BUTTON: Color = Color::srgb(0.15, 0.15, 0.15);
const HOVERED_BUTTON: Color = Color::srgb(0.25, 0.25, 0.25);
const PRESSED_BUTTON: Color = Color::srgb(0.35, 0.35, 0.35);

#[derive(Component)]
enum MenuButton {
    Start,
    Credits,
    #[cfg(not(target_arch = "wasm32"))]
    Exit,
}

#[derive(Component)]
struct CreditsOverlay;

fn setup_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Root container.
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            DespawnOnExit(Sections::Menu),
        ))
        .with_children(|parent| {
            // Logo image.
            parent.spawn((
                ImageNode::new(asset_server.load("header.png")),
                Node {
                    width: Val::Px(514.0),
                    height: Val::Px(73.0),
                    margin: UiRect::bottom(Val::Px(32.0)),
                    ..default()
                },
            ));

            // Start button.
            spawn_button(parent, "Start", MenuButton::Start);

            // Credits button.
            spawn_button(parent, "Credits", MenuButton::Credits);

            // Exit button (native only).
            #[cfg(not(target_arch = "wasm32"))]
            spawn_button(parent, "Exit", MenuButton::Exit);
        });
}

fn spawn_button(parent: &mut ChildSpawnerCommands, label: &str, marker: MenuButton) {
    parent
        .spawn((
            marker,
            Button,
            Node {
                width: Val::Px(200.0),
                height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            BackgroundColor(NORMAL_BUTTON),
        ))
        .with_children(|btn| {
            btn.spawn((
                Text::new(label),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn button_visuals(
    mut query: Query<
        (&Interaction, &mut BackgroundColor, &mut BorderColor),
        (Changed<Interaction>, With<MenuButton>),
    >,
) {
    for (interaction, mut bg, mut border) in &mut query {
        match *interaction {
            Interaction::Pressed => {
                *bg = PRESSED_BUTTON.into();
                *border = BorderColor::all(Color::WHITE);
            }
            Interaction::Hovered => {
                *bg = HOVERED_BUTTON.into();
                *border = BorderColor::all(Color::WHITE);
            }
            Interaction::None => {
                *bg = NORMAL_BUTTON.into();
                *border = BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.3));
            }
        }
    }
}

fn button_actions(
    query: Query<(&Interaction, &MenuButton), Changed<Interaction>>,
    mut next_state: ResMut<NextState<Sections>>,
    mut commands: Commands,
    #[cfg(not(target_arch = "wasm32"))] mut exit: MessageWriter<AppExit>,
) {
    for (interaction, button) in &query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            MenuButton::Start => {
                next_state.set(Sections::Chase);
            }
            MenuButton::Credits => {
                spawn_credits_overlay(&mut commands);
            }
            #[cfg(not(target_arch = "wasm32"))]
            MenuButton::Exit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

fn spawn_credits_overlay(commands: &mut Commands) {
    commands
        .spawn((
            CreditsOverlay,
            DespawnOnExit(Sections::Menu),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(16.0),
                position_type: PositionType::Absolute,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 99.)),
            GlobalZIndex(200),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Credits"),
                TextFont {
                    font_size: 36.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            let lines = [
                "A game by TM Storey",
                "",
                "Thanks to Quaternius for many assets and animations",
                "",
                "Made with Bevy",
                "For Bevy Jam #7",
            ];
            for line in lines {
                parent.spawn((
                    Text::new(line),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgba(0.8, 0.8, 0.8, 1.0)),
                ));
            }

            // Back button.
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(120.0),
                        height: Val::Px(40.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        margin: UiRect::top(Val::Px(24.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.3)),
                    BackgroundColor(NORMAL_BUTTON),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("Back"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

fn credits_back(
    mut commands: Commands,
    overlay: Query<Entity, With<CreditsOverlay>>,
    buttons: Query<&Interaction, (Changed<Interaction>, Without<MenuButton>)>,
) {
    // The Back button in the credits overlay has no MenuButton marker.
    for interaction in &buttons {
        if *interaction == Interaction::Pressed {
            for entity in &overlay {
                commands.entity(entity).despawn();
            }
        }
    }
}
