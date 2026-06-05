pub mod command;
pub mod matcher;
pub mod registry;
pub mod render;
pub mod state;

pub use command::{AppAction, CommandAction, CommandId, ModalKind};
pub use state::{CommandPaletteState, PaletteMode};

#[derive(Debug, Clone, Default)]
pub struct CommandPalette {
    pub state: CommandPaletteState,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests;
