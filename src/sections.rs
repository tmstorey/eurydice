/// Game sections and shared plot state.
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum Sections {
    #[default]
    Menu,
    Chase,
    Underworld,
    Stairs,
    Awaken,
}

/// Flags that persist across section transitions to drive plot branching.
#[derive(Resource, Default)]
pub struct PlotFlags {
    pub player_looked_behind: bool,
    pub chevron_appeared: bool,
}
