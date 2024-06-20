use std::{
	os::unix::fs::MetadataExt,
	sync::{
		atomic::{AtomicUsize, Ordering},
		Arc
	}
};

use chrono::DateTime;
use crossterm::event::{Event, KeyEvent, KeyModifiers};
use dir::DirViewer;
use filemagic::Magic;
use flume::Sender;
use ratatui::{
	prelude::Rect,
	style::{Modifier, Style},
	text::{Line, Span, Text},
	widgets::{Paragraph, Wrap},
	Frame
};
use text::TextViewer;
use tracing::info;

use super::{Component, EventHandleResult};
use crate::fileinfo::{FileInfo, FilePath};

mod dir;
mod image;
mod text;

#[derive(Debug)]
pub enum ViewType {
	Text(Paragraph<'static>),
	Directory(Vec<Line<'static>>),
	Unknown
}

#[derive(Debug)]
pub struct ViewMsg {
	pub fileinfo: FileInfo,
	pub body: ViewType,
	pub attr: Option<Paragraph<'static>>
}

pub struct FileViewer {
	file: Option<(ViewMsg, u16)>,
	text_viewer: Arc<TextViewer>,
	dir_viewer: Arc<DirViewer>,

	magic: Option<Arc<Magic>>,
	ticket: Arc<AtomicUsize>,
	out_tx: Sender<ViewMsg>
}

#[derive(Clone, Debug)]
enum FileInfoHandleErr {
	TextReadErr(String),
	Cancelled,
	NotImplement,
	Error(String)
}

impl Component for FileViewer {
	type MsgIn = String;

	fn draw(
		&mut self,
		f: &mut Frame,
		rect: &Rect,
		changed: bool
	) -> chin_tools::wrapper::anyhow::RResult<()> {
		if let Some((msg, cursor)) = self.file.as_ref() {
			match &msg.body {
				ViewType::Text(_) => {
					self.text_viewer.draw(msg, *cursor, f, rect);
				}
				ViewType::Directory(_) => self.dir_viewer.draw(msg, *cursor, f, rect),
				ViewType::Unknown => {}
			}
		}

		Ok(())
	}

	fn _widget(&self, _rect: &Rect, _changed: bool) -> impl ratatui::prelude::Widget {
		Span::raw("")
	}

	fn handle_event(&mut self, _event: &Event) -> EventHandleResult {
		if *_event
			== Event::Key(KeyEvent::new(
				crossterm::event::KeyCode::Down,
				KeyModifiers::ALT
			)) {
			self.file.as_mut().map(|e| e.1 = e.1.saturating_add(1));
		} else if *_event
			== Event::Key(KeyEvent::new(
				crossterm::event::KeyCode::Up,
				KeyModifiers::ALT
			)) {
			self.file.as_mut().map(|e| e.1 = e.1.saturating_sub(1));
		}

		EventHandleResult::all()
	}
}

impl FileViewer {
	pub fn new(out_tx: Sender<ViewMsg>) -> Self {
		// open a new configuration with flags
		let magic = filemagic::magic!().ok().map(|e| Arc::new(e));

		Self {
			file: None,
			text_viewer: Arc::new(TextViewer::new()),
			dir_viewer: Arc::new(DirViewer::new()),
			magic,
			ticket: Arc::new(AtomicUsize::new(0)),
			out_tx
		}
	}

	pub fn handle_file(&mut self, fileinfo: Option<&FilePath>) {
		info!("handle file {:?}", fileinfo.map(|e| e.path()));
		if let None = fileinfo {
			if self.file.is_some() {
				self.file.take();
			}

			return;
		}

		let fileinfo = fileinfo.unwrap();

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

		if let Some(m) = self.magic.as_ref() {
			fileinfo.desc = m.file(fileinfo.path()).ok();
		}

		let sender = self.out_tx.clone();
		let ticket_holder = self.ticket.clone();
		let ticket = ticket_holder
			.fetch_add(1, Ordering::Relaxed)
			.wrapping_add(1);

		let signal = move || ticket != ticket_holder.load(Ordering::Relaxed);

		let text_handler = self.text_viewer.clone();
		let dir_walker = self.dir_viewer.clone();

		tokio::spawn(async move {
			let mut fileinfo = fileinfo;
			fileinfo.metadata = fileinfo.path.pathbuf.metadata().map_err(|e| e.to_string());

			match &fileinfo.metadata {
				Ok(metadata) => {
					let msg = if metadata.is_dir() {
						dir_walker.handle_fileinfo(fileinfo, signal).await
					} else if metadata.is_file() {
						text_handler.handle_fileinfo(fileinfo, signal).await
					} else {
						Err(FileInfoHandleErr::NotImplement)
					};

					if let Ok(msg) = msg {
						let _ = sender.send_async(msg).await;
					}
				}

				Err(_) => {}
			}
		});
	}

	pub fn set_view(&mut self, msg: ViewMsg) {
		if self
			.file
			.as_ref()
			.map_or(true, |(m, _)| m.fileinfo.path() == msg.fileinfo.path())
		{
			self.file.replace((msg, 0));
		}
	}
}

trait Viewer {
	async fn handle_fileinfo<F>(
		&self,
		fileinfo: FileInfo,
		cancel_signal: F
	) -> Result<ViewMsg, FileInfoHandleErr>
	where
		F: Fn() -> bool + Clone;

	fn handle_event(&mut self, event: Event);

	fn draw(&self, view_msg: &ViewMsg, show_cursor: u16, f: &mut Frame, rect: &Rect);
}

fn regular_file_attrs(fi: &FileInfo) -> Option<Paragraph<'static>> {
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

fn tui_line(title: &'static str, content: &str) -> Line<'static> {
	Line::from(vec![
		Span {
			content: title.into(),
			style: ratatui::style::Stylize::bold(Style::new())
		},
		ratatui::text::Span::styled(String::from(content), Style::new()),
	])
}
