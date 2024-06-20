use std::{
	os::unix::fs::MetadataExt,
	sync::{
		atomic::{AtomicUsize, Ordering},
		Arc
	}
};

use chrono::DateTime;
use ratatui::{
	layout::{Constraint, Layout, Rect},
	style::Style,
	text::{Line, Span, Text},
	widgets::Paragraph,
	Frame
};

use super::{tui_line, ViewMsg, Viewer};
use crate::{dirwalker, fileinfo::FileInfo};

pub struct DirViewer {}

impl DirViewer {
	pub fn new() -> Self {
		Self {}
	}
}

impl Viewer for DirViewer {
	fn reset(&mut self) {}

	async fn handle_fileinfo<F>(
		&self,
		fileinfo: FileInfo,
		cancel_signal: F,
		text: Option<String>
	) -> Option<ViewMsg>
	where
		F: Fn() -> bool + Clone
	{
		let mut attr_vec = Vec::new();
		let mut filenames = vec![];

		let text = dirwalker::read_dir2(fileinfo.path.path(), &cancel_signal).await;
		match text {
			Ok((fns, count)) => {
				attr_vec.push(tui_line("Size: ", format!("{}", count).as_str()));
				filenames = fns
					.iter()
					.map(|e| Line::from(Span::styled(String::from(e), Style::new())))
					.collect();
			}
			Err(err) => {
				attr_vec.push(tui_line("Unable to read", err.to_string().as_str()));
			}
		};

		if let Ok(md) = fileinfo.metadata.as_ref() {
			if let Some(t) = DateTime::from_timestamp(md.mtime(), 0) {
				attr_vec.push(tui_line(
					"MTime: ",
					t.naive_local()
						.format("%Y-%m-%d %H:%M:%S")
						.to_string()
						.as_str()
				));
			}
		}

		match fileinfo.desc.as_ref() {
			Some(desc) => attr_vec.push(tui_line("Type: ", &desc)),
			None => {}
		};

		if cancel_signal() {
			None
		} else {
			Some(ViewMsg {
				fileinfo,
				body: super::ViewType::Directory(filenames),
				attr: Some(Paragraph::new(Text::from(attr_vec)))
			})
		}
	}

	fn handle_event(&mut self, event: crossterm::event::Event) {}

	fn draw(&self, view_msg: &ViewMsg, cursor: usize, f: &mut Frame, rect: &Rect) {
		let attrs = view_msg.attr.as_ref();
		let attrs_height = attrs
			.as_ref()
			.map(|e| e.line_count(rect.width))
			.unwrap_or(0)
			.clamp(0, 5) as u16;

		let tb =
			Layout::vertical([Constraint::Fill(1), Constraint::Max(attrs_height)]).split(*rect);

		attrs.map(|e| f.render_widget(e.clone(), tb[1]));

		match &view_msg.body {
			super::ViewType::Directory(entries) => {
				tracing::info!("not directory dir {}", entries.len());
				f.render_widget(Paragraph::new(Text::from(entries.clone())), tb[0])
			}

			_ => {
				tracing::info!("not directory dir");
			}
		}
	}
}
