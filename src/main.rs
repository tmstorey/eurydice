// Main
mod awaken;
mod chase;
mod dream;
mod menu;
mod npc;
mod player;
mod sections;
mod terrain;
mod transition;
mod stairs;
mod underworld;

use bevy::prelude::*;
use awaken::AwakenPlugin;
use chase::ChasePlugin;
use dream::DreamPlugin;
use menu::MenuPlugin;
use npc::NpcPlugin;
use player::PlayerPlugin;
use sections::{PlotFlags, Sections};
use stairs::StairsPlugin;
use terrain::TerrainPlugin;
use transition::TransitionPlugin;
use underworld::UnderworldPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<Sections>()
        .init_resource::<PlotFlags>()
        .add_plugins((
            MenuPlugin,
            PlayerPlugin,
            TerrainPlugin,
            DreamPlugin,
            NpcPlugin,
            ChasePlugin,
            UnderworldPlugin,
            StairsPlugin,
            AwakenPlugin,
            TransitionPlugin,
        ))
        .run();
}
