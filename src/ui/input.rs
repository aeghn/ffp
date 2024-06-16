use crossterm::event::Event;
use flume::Sender;
use ratatui::{
	layout::Rect,
	widgets::{Paragraph, Widget}
};
use tracing::info;

use super::{Component, ConsumeState, NeedRedraw};

#[derive(Debug)]
pub enum InputIn {
	Clear,
	WidthChange(usize),
	Event(Event)
}

#[derive(Debug)]
pub enum InputOut {
	Input(String)
}

pub struct Input {
	out_tx: flume::Sender<InputOut>,
	input: String,
	cursor_position: usize,
	show_start: usize,
	rect: Rect
}

impl Input {
	pub fn new(rect: Rect, out_tx: Sender<InputOut>) -> Input {
		Input {
			out_tx,
			input: "".to_string(),
			cursor_position: 0,
			show_start: 0,
			rect
		}
	}

	fn content_width(&self) -> u16 {
		self.rect.width
	}

	fn send_input(&self) {
		self.out_tx
			.send(InputOut::Input(self.input.clone()))
			.unwrap();
	}

	fn move_cursor_left(&mut self) {
		info!("cursor_position {:?}", self.cursor_position);
		self.cursor_position = self
			.cursor_position
			.saturating_sub(1)
			.clamp(0, self.input.len());
		info!("cursor_position {:?}", self.cursor_position);
		if self.cursor_position <= self.show_start {
			self.show_start = self.show_start.saturating_sub(1);
		}
	}

	fn move_cursor_right(&mut self) {
		self.cursor_position = self
			.cursor_position
			.saturating_add(1)
			.clamp(0, self.input.len());
		if self.cursor_position >= self.show_start + self.content_width() as usize - 1 {
			self.show_start = self.show_start.saturating_add(1);
		}
	}

	fn enter_char(&mut self, new_char: char) {
		self.input.insert(self.cursor_position, new_char);
		self.move_cursor_right();
		self.send_input();
	}

	fn move_start(&mut self) {
		self.cursor_position = 0;
		self.show_start = 0
	}

	fn move_end(&mut self) {
		self.cursor_position = self.input.len() - 1;
		self.show_start = self
			.cursor_position
			.saturating_sub(self.content_width() as usize)
	}

	fn delete_char(&mut self) {
		let is_not_cursor_leftmost = self.cursor_position != 0;
		if is_not_cursor_leftmost {
			// Method "remove" is not used on the saved text for deleting the selected char.
			// Reason: Using remove on String works on bytes instead of the chars.
			// Using remove would require special care because of char boundaries.

			let current_index = self.cursor_position;
			let from_left_to_current_index = current_index - 1;

			// Getting all characters before the selected character
			let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
			// Getting all characters after selected character.
			let after_char_to_delete = self.input.chars().skip(current_index);

			// Put all characters together except the selected one.
			// By leaving the selected one out, it is forgotten and therefore deleted.
			self.input = before_char_to_delete.chain(after_char_to_delete).collect();
			self.send_input();

			self.show_start = self.show_start.saturating_sub(1 as usize);
			self.move_cursor_left();
		}
	}
}

impl Component for Input {
	type MsgIn = InputIn;
	fn draw(&self, f: &mut ratatui::Frame) -> chin_tools::wrapper::anyhow::RResult<()> {
		f.render_widget(self.widget(), self.rect.clone());
		f.set_cursor(
			self.rect.x + (self.cursor_position - self.show_start) as u16,
			self.rect.y
		);
		Ok(())
	}

	fn widget(&self) -> impl Widget {
		Paragraph::new(&self.input[self.show_start..])
	}

	fn show(&mut self) {}

	fn hide(&mut self) {}

	fn handle_event(&mut self, event: Event) -> (NeedRedraw, ConsumeState) {
		match event {
			Event::Key(key) => {
				/* 				if key.modifiers != KeyModifiers::NONE || key.modifiers != KeyModifiers::SHIFT {
					return false;
				} */

				match key.code {
					crossterm::event::KeyCode::Backspace => {
						self.delete_char();
						(NeedRedraw::Yes, ConsumeState::Consumed)
					}
					crossterm::event::KeyCode::Left => {
						self.move_cursor_left();
						(NeedRedraw::Yes, ConsumeState::Consumed)
					}
					crossterm::event::KeyCode::Right => {
						self.move_cursor_right();
						(NeedRedraw::Yes, ConsumeState::Consumed)
					}
					crossterm::event::KeyCode::Home => {
						self.move_start();
						(NeedRedraw::Yes, ConsumeState::Consumed)
					}
					crossterm::event::KeyCode::End => {
						self.move_end();
						(NeedRedraw::Yes, ConsumeState::Consumed)
					}
					crossterm::event::KeyCode::Char(c) => {
						self.enter_char(c);
						(NeedRedraw::Yes, ConsumeState::Consumed)
					}
					_ => (NeedRedraw::No, ConsumeState::NotConsumed)
				}
			}
			_ => (NeedRedraw::No, ConsumeState::NotConsumed)
		}
	}

	fn handle_msg(&mut self, msg: Self::MsgIn) {
		match msg {
			InputIn::Clear => {}
			InputIn::WidthChange(_) => {}
			InputIn::Event(_) => {}
		}
	}
}
