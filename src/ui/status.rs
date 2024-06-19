use ratatui::{layout::Rect, widgets::Paragraph};

use super::{Component};
use crate::dirwalker::FindType;

pub enum StatusIn {
	CWD(String),
	ShowType(FindType),
	ShowHide(bool),
	Total(usize),
	FilterSize(usize)
}

pub struct Status {
	cwd: String,
	show_type: FindType,
	show_hide: bool,
	total: usize,
	filter_size: usize
}

impl Status {
	pub fn new(cwd: &str) -> Self {
		Self {
			cwd: cwd.to_string(),
			show_hide: Default::default(),
			show_type: Default::default(),
			filter_size: 0,
			total: 0
		}
	}

	pub fn set_total(&mut self, num: usize) {
		self.total = num;
	}

	pub fn set_filter_count(&mut self, num: usize) {
		self.filter_size = num;
	}
}

impl Component for Status {
	type MsgIn = StatusIn;

	fn draw(
		&mut self,
		f: &mut ratatui::Frame,
		rect: &Rect,
		changed: bool
	) -> chin_tools::wrapper::anyhow::RResult<()> {
		f.render_widget(self._widget(rect, changed), rect.clone());
		Ok(())
	}

	fn _widget(&self, rect: &Rect, changed: bool) -> impl ratatui::prelude::Widget {
		let find_type = match self.show_type {
			FindType::LS => "L",
			FindType::FIND => "F"
		};

		let hide_type = if self.show_hide { "[H]" } else { "" };

		Paragraph::new(format!(
			"-- [{}]{} {}/{} {}",
			find_type, hide_type, self.filter_size, self.total, self.cwd
		))
	}

	fn handle_msg(&mut self, msg: Self::MsgIn) {
		match msg {
			StatusIn::CWD(cwd) => {
				self.cwd = cwd;
			}
			StatusIn::ShowType(st) => {
				self.show_type = st;
			}
			StatusIn::ShowHide(sh) => {
				self.show_hide = sh;
			}
			StatusIn::Total(total) => {
				self.total = total;
			}
			StatusIn::FilterSize(f) => {
				self.filter_size = f;
			}
		}
	}
}
