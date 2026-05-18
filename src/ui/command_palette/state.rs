use super::command::Command;
use super::matcher::CommandMatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteMode {
    Fuzzy,
    Colon,
}

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    pub is_open: bool,
    pub mode: PaletteMode,
    pub input: String,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub results: Vec<CommandMatch>,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            is_open: false,
            mode: PaletteMode::Fuzzy,
            input: String::new(),
            selected_index: 0,
            scroll_offset: 0,
            results: Vec::new(),
        }
    }
}

impl CommandPaletteState {
    pub fn open_fuzzy(&mut self) {
        self.is_open = true;
        self.mode = PaletteMode::Fuzzy;
        self.input.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.refresh_results();
    }

    pub fn open_colon(&mut self) {
        self.is_open = true;
        self.mode = PaletteMode::Colon;
        self.input.clear();
        self.input.push(':');
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.refresh_results();
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.input.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.results.clear();
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        if self.mode == PaletteMode::Colon {
            self.input.push(':');
        }
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.refresh_results();
    }

    pub fn input_char(&mut self, c: char) {
        self.input.push(c);
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.refresh_results();
    }

    pub fn backspace(&mut self) {
        if self.mode == PaletteMode::Colon && self.input.len() <= 1 {
            return;
        }
        self.input.pop();
        self.selected_index = 0;
        self.scroll_offset = 0;
        self.refresh_results();
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
        self.update_scroll_offset();
    }

    pub fn move_down(&mut self) {
        if self.selected_index + 1 < self.results.len() {
            self.selected_index += 1;
        }
        self.update_scroll_offset();
    }

    fn update_scroll_offset(&mut self) {
        let visible_count = 10;
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_count {
            self.scroll_offset = self.selected_index.saturating_sub(visible_count - 1);
        }
    }

    pub fn selected_command(&self) -> Option<&Command> {
        self.results.get(self.selected_index).map(|m| &m.command)
    }

    fn refresh_results(&mut self) {
        self.results = super::matcher::match_commands(&self.input);
    }
}
