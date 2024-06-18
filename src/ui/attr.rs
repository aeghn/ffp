use std::{fs::Metadata, os::unix::fs::MetadataExt};

use anyhow::Ok;
use ratatui::{
	layout::Rect,
	style::Style,
	text::{Line, Span},
	widgets::{Paragraph, Wrap}
};

use super::Component;

pub struct FileAttr {
	atime: String,
	size: String,
	desc: String,
	rect: Rect
}

impl FileAttr {
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

impl Component for FileAttr {
	type MsgIn = ();

	fn draw(
		&mut self,
		f: &mut ratatui::Frame,
		rect: &Rect,
		changed: bool
	) -> chin_tools::wrapper::anyhow::RResult<()> {
		f.render_widget(self._widget(rect, changed), self.rect.clone());
		Ok(())
	}

	fn _widget(&self, rect: &Rect, changed: bool) -> impl ratatui::prelude::Widget {
		Paragraph::new(Line::from(vec![
			Span {
				content: "Type: ".into(),
				style: ratatui::style::Stylize::bold(Style::new())
			},
			Span {
				content: self.desc.clone().into(),
				style: ratatui::style::Stylize::italic(Style::new())
			},
		]))
		.wrap(Wrap { trim: true })
	}
}
