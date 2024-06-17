use std::{fs::Metadata, os::unix::fs::MetadataExt};

use anyhow::Ok;
use crossterm::style::Stylize;
use file_format::FileFormat;
use ratatui::{
	layout::Rect,
	style::Style,
	text::{Line, Span, Text},
	widgets::{Paragraph, Wrap}
};

use super::{Component, ConsumeState, NeedRedraw};

pub struct FileSkim {
	atime: String,
	size: String,
	desc: String,
	rect: Rect
}

impl FileSkim {
	pub fn new(metadata: Option<&Metadata>, file_format: Option<&String>, rect: Rect) -> Self {
		Self {
			atime: metadata
				.as_ref()
				.map_or("".to_string(), |e| e.atime().to_string()),
			size: metadata
				.as_ref()
				.map_or("".to_string(), |e| e.size().to_string()),
			desc: file_format.map_or("Unknown File Format".to_string(), |e| e.to_string()),
			rect
		}
	}
}

impl Component for FileSkim {
	type MsgIn = ();

	fn draw(&self, f: &mut ratatui::Frame) -> chin_tools::wrapper::anyhow::RResult<()> {
		f.render_widget(self.widget(), self.rect.clone());
		Ok(())
	}

	fn widget(&self) -> impl ratatui::prelude::Widget {
		Paragraph::new(Line::from(vec![
			Span {
				content: "Type: ".into(),
				style: ratatui::style::Stylize::bold(Style::new())
			},
			self.desc.clone().into(),
		]))
		.wrap(Wrap { trim: true })
	}

	fn show(&mut self) {
		todo!()
	}

	fn hide(&mut self) {
		todo!()
	}

	fn handle_msg(&mut self, msg: Self::MsgIn) {}

	fn handle_event(
		&mut self,
		event: crossterm::event::Event
	) -> (super::NeedRedraw, super::ConsumeState) {
		(NeedRedraw::No, ConsumeState::NotConsumed)
	}
}
