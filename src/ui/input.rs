use crossterm::event::Event;
use flume::Sender;
use ratatui::{
	layout::Rect,
	widgets::{Paragraph, Widget}
};

use super::{Component, ConsumeP, RedrawP};

#[derive(Debug)]
pub enum InputIn {
	Clear,
	Event(Event)
}

#[derive(Debug)]
pub enum InputOut {
	Input(String)
}

pub enum InputMove {
	Start,
	Left,
	Right,
	End,
	Nil
}

pub struct Input {
	out_tx: flume::Sender<InputOut>,
	input: String,

	cursor_position: usize,
	show_start: usize,
	input_move: InputMove
}

impl Input {
	pub fn new(out_tx: Sender<InputOut>) -> Input {
		Input {
			out_tx,
			input: "".to_string(),
			cursor_position: 0,
			show_start: 0,
			input_move: InputMove::Nil
		}
	}

	fn send_input(&self) {
		self.out_tx
			.send(InputOut::Input(self.input.clone()))
			.unwrap();
	}

	fn move_cursor_left(&mut self) {
		self.cursor_position = self
			.cursor_position
			.saturating_sub(1)
			.clamp(0, self.input.len());
		self.input_move = InputMove::Left;
	}

	fn move_cursor_right(&mut self) {
		self.cursor_position = self
			.cursor_position
			.saturating_add(1)
			.clamp(0, self.input.len());
		self.input_move = InputMove::Right;
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

	fn move_end(&mut self, width: usize) {
		self.cursor_position = self.input.len() - 1;
		self.show_start = self.cursor_position.saturating_sub(width as usize)
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
	fn draw(
		&mut self,
		f: &mut ratatui::Frame,
		rect: &Rect,
		changed: bool
	) -> chin_tools::wrapper::anyhow::AResult<()> {
		let width = rect.width as usize;
		match self.input_move {
			InputMove::Start => {
				self.move_start();
			}
			InputMove::Left =>
				if self.cursor_position - self.show_start > width {
					self.show_start = self.cursor_position.saturating_sub(width);
				} else {
					if self.cursor_position <= self.show_start {
						self.show_start = self.show_start.saturating_sub(1);
					}
				},

			InputMove::Right =>
				if self.cursor_position - self.show_start > width {
					self.show_start = self.cursor_position.saturating_sub(width);
				} else {
					if self.cursor_position >= self.show_start + rect.width as usize - 1 {
						self.show_start = self.show_start.saturating_add(1);
					}
				},
			InputMove::End => {
				self.move_end(rect.width.into());
			}
			InputMove::Nil => {}
		}

		f.render_widget(self._widget(rect, changed), rect.clone());
		f.set_cursor(
			rect.x + (self.cursor_position - self.show_start) as u16,
			rect.y
		);
		Ok(())
	}

	fn _widget(&self, rect: &Rect, changed: bool) -> impl Widget {
		Paragraph::new(&self.input[self.show_start..])
	}

	fn handle_event(&mut self, event: Event) -> (RedrawP, ConsumeP) {
		match event {
			Event::Key(key) => {
				/* 				if key.modifiers != KeyModifiers::NONE || key.modifiers != KeyModifiers::SHIFT {
					return false;
				} */

				match key.code {
					crossterm::event::KeyCode::Backspace => {
						self.delete_char();
						(RedrawP::Yes, ConsumeP::Yes)
					}
					crossterm::event::KeyCode::Left => {
						self.move_cursor_left();

						(RedrawP::Yes, ConsumeP::Yes)
					}
					crossterm::event::KeyCode::Right => {
						self.move_cursor_right();

						(RedrawP::Yes, ConsumeP::Yes)
					}
					crossterm::event::KeyCode::Home => {
						self.input_move = InputMove::Start;
						(RedrawP::Yes, ConsumeP::Yes)
					}
					crossterm::event::KeyCode::End => {
						self.input_move = InputMove::End;
						(RedrawP::Yes, ConsumeP::Yes)
					}
					crossterm::event::KeyCode::Char(c) => {
						self.enter_char(c);
						(RedrawP::Yes, ConsumeP::Yes)
					}
					_ => (RedrawP::No, ConsumeP::No)
				}
			}
			_ => (RedrawP::No, ConsumeP::No)
		}
	}
}
