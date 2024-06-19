use std::{
	os::unix::fs::MetadataExt,
	sync::{
		atomic::{AtomicUsize, Ordering},
		Arc
	}
};

use chrono::DateTime;
use crossterm::event::Event;
use flume::Sender;
use magic::{cookie::Load, Cookie};
use ratatui::{
	prelude::Rect,
	style::Style,
	text::{Line, Span, Text},
	widgets::{Paragraph, Wrap},
	Frame
};
use text::TextViewer;

use crate::{
	dirwalker::read_first_n_chars,
	fileinfo::{FileInfo, FilePath}
};

pub mod attr;
pub mod text;

pub enum ViewType {
	Text(Paragraph<'static>),
	Directory,
	Unknown
}

pub struct ViewMsg {
	pub fileinfo: FileInfo,
	pub body: ViewType,
	pub attr: Option<Paragraph<'static>>
}

pub struct FileViewer {
	file: Option<(ViewMsg, usize)>,
	text_viewer: Arc<TextViewer>,
	magic: Option<Arc<Cookie<Load>>>,
	ticket: Arc<AtomicUsize>,
	out_tx: Sender<ViewMsg>
}

impl FileViewer {
	pub fn new(out_tx: Sender<ViewMsg>) -> Self {
		// open a new configuration with flags
		let cookie = magic::Cookie::open(magic::cookie::Flags::ERROR)
			.map(|cookie| {
				// load the system's default database
				let database = &Default::default();
				cookie.load(database).ok()
			})
			.ok();
		let cookie = match cookie {
			Some(e) => e,
			None => None
		}
		.map(|e| Arc::new(e));

		Self {
			file: None,
			text_viewer: Arc::new(TextViewer::new()),
			magic: cookie,
			ticket: Arc::new(AtomicUsize::new(0)),
			out_tx
		}
	}

	pub fn handle_file(&mut self, fileinfo: &FilePath) {
		if self
			.file
			.as_ref()
			.map_or(false, |e| e.0.fileinfo.path() == fileinfo.path())
		{
			return;
		} else {
			self.file.take();
		}

		let mut fileinfo: FileInfo = fileinfo.clone().into();

		let magic = self.magic.clone();
		magic.map(|m| fileinfo.desc = m.file(fileinfo.path()).ok());

		let sender = self.out_tx.clone();
		let ticket_holder = self.ticket.clone();
		let text_handler = self.text_viewer.clone();

		tokio::spawn(async move {
			let ticket = ticket_holder.load(Ordering::Relaxed);
			let mut fileinfo = fileinfo;
			fileinfo.metadata = fileinfo.path.pathbuf.metadata().map_err(|e| e.to_string());

			let msg = match read_first_n_chars(fileinfo.path(), 5000).await {
				Ok(text) =>
					text_handler
						.handle_fileinfo(fileinfo, ticket, ticket_holder, Some(text))
						.await,
				Err(_) => None
			};

			if let Some(msg) = msg {
				sender.send(msg);
			}
		});
	}

	pub fn set_view(&mut self, msg: ViewMsg) {
		if self
			.file
			.as_ref()
			.map_or(true, |(m, c)| m.fileinfo.path() == msg.fileinfo.path())
		{
		self.file.replace((msg, 0));
	}
	}

	pub fn view(&mut self, frame: &mut Frame, rect: &Rect) {
		if let Some((msg, cursor)) = self.file.as_ref() {
			match &msg.body {
				ViewType::Text(text) => {
					self.text_viewer.draw(msg, *cursor, frame, rect);
				}
				ViewType::Directory => {}
				ViewType::Unknown => {}
			}
		}
	}
}

pub trait Viewer {
	fn reset(&mut self);

	async fn handle_fileinfo(
		&self,
		fileinfo: FileInfo,
		ticket: usize,
		ticket_holder: Arc<AtomicUsize>,
		text: Option<String>
	) -> Option<ViewMsg>;

	fn handle_event(&mut self, event: Event);

	fn draw(&self, view_msg: &ViewMsg, show_cursor: usize, f: &mut Frame, rect: &Rect);

	fn attrs(fi: &FileInfo) -> Option<Paragraph<'static>> {
		let mut vec = Vec::new();
		if let Ok(md) = fi.metadata.as_ref() {
			vec.push(tui_line(
				"Size: ",
				human_bytes::human_bytes(md.len() as f64).as_str()
			));
			if let Some(t) = DateTime::from_timestamp(md.mtime(), 0) {
				vec.push(tui_line(
					"MTime: ",
					t.naive_local()
						.format("%Y-%m-%d %H:%M:%S")
						.to_string()
						.as_str()
				));
			}
		}

		match fi.desc.as_ref() {
			Some(desc) => vec.push(tui_line("Type: ", &desc)),
			None => {}
		};

		if vec.is_empty() {
			None
		} else {
			Some(Paragraph::new(Text::from(vec)).wrap(Wrap { trim: true }))
		}
	}
}

fn tui_line(title: &'static str, content: &str) -> Line<'static> {
	Line::from(vec![
		Span {
			content: title.into(),
			style: ratatui::style::Stylize::bold(Style::new())
		},
		ratatui::text::Span::styled(String::from(content), Style::new()),
	])
}
