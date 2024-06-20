use std::rc::Rc;

use ratatui::style::{Color, Style};

pub type SharedTheme = Rc<Theme>;

#[derive(Debug, Clone)]
pub struct Theme {
	selection_bg: Color,
	selection_fg: Color,
	disabled_fg: Color,
	command_fg: Color
}

impl Default for Theme {
	fn default() -> Self {
		Self {
			selection_bg: Color::Blue,
			selection_fg: Color::Yellow,
			disabled_fg: Default::default(),
			command_fg: Color::LightYellow
		}
	}
}

impl Theme {
	pub fn scroll_bar_pos(&self) -> Style {
		Style::default().fg(self.selection_bg)
	}

	pub fn block(&self, focus: bool) -> Style {
		Style::default()
	}

	pub fn text(&self, enabled: bool, selected: bool) -> Style {
		match (enabled, selected) {
			(false, false) => Style::default().fg(self.disabled_fg),
			(false, true) => Style::default().bg(self.selection_bg),
			(true, false) => Style::default().fg(self.command_fg),
			(true, true) => Style::default().fg(self.command_fg).bg(self.selection_bg)
		}
	}
}
