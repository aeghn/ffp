use chin_tools::wrapper::anyhow::RResult;
use crossterm::event::Event;
use ratatui::{
	layout::{Constraint, Direction, Layout, Rect},
	widgets::Widget,
	Frame
};

pub mod finder;
pub mod input;
pub mod status;
pub mod theme;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NeedRedraw {
	Yes,
	No,
	Unsure
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConsumeState {
	Consumed,
	NotConsumed
}

#[derive(Copy, Clone)]
pub struct Size {
	pub width: u16,
	pub height: u16
}

impl Size {
	pub const fn new(width: u16, height: u16) -> Self {
		Self { width, height }
	}
}

impl From<Rect> for Size {
	fn from(r: Rect) -> Self {
		Self {
			width: r.width,
			height: r.height
		}
	}
}

/// use layouts to create a rects that
/// centers inside `r` and sizes `percent_x`/`percent_x` of `r`
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
	let popup_layout = Layout::default()
		.direction(Direction::Vertical)
		.constraints(
			[
				Constraint::Percentage((100 - percent_y) / 2),
				Constraint::Percentage(percent_y),
				Constraint::Percentage((100 - percent_y) / 2)
			]
			.as_ref()
		)
		.split(r);

	Layout::default()
		.direction(Direction::Horizontal)
		.constraints(
			[
				Constraint::Percentage((100 - percent_x) / 2),
				Constraint::Percentage(percent_x),
				Constraint::Percentage((100 - percent_x) / 2)
			]
			.as_ref()
		)
		.split(popup_layout[1])[1]
}

/// makes sure Rect `r` at least stays as big as min and not bigger than max
pub fn rect_inside(min: Size, max: Size, r: Rect) -> Rect {
	let new_width = if min.width > max.width {
		max.width
	} else {
		r.width.clamp(min.width, max.width)
	};

	let new_height = if min.height > max.height {
		max.height
	} else {
		r.height.clamp(min.height, max.height)
	};

	let diff_width = new_width.saturating_sub(r.width);
	let diff_height = new_height.saturating_sub(r.height);

	Rect::new(
		r.x.saturating_sub(diff_width / 2),
		r.y.saturating_sub(diff_height / 2),
		new_width,
		new_height
	)
}

pub trait Component {
	type MsgIn;

	fn draw(&self, f: &mut Frame) -> RResult<()>;
	fn widget(&self) -> impl Widget;

	fn is_visible(&self) -> bool {
		true
	}
	fn show(&mut self);
	fn hide(&mut self);

	fn handle_msg(&mut self, msg: Self::MsgIn);
	fn handle_event(&mut self, event: Event) -> (NeedRedraw, ConsumeState);
}
