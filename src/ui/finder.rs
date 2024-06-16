use std::{
	borrow::Cow,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc, RwLock
	},
	thread
};

use crossterm::event::Event;
use flume::Sender;
use fuzzy_matcher::FuzzyMatcher;
use ratatui::{
	layout::Rect,
	text::{Line, Span},
	widgets::{Block, Borders},
	Frame
};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use tracing::error;
use unicode_segmentation::UnicodeSegmentation;

use super::{theme::SharedTheme, Component, ConsumeState, NeedRedraw};
use crate::{
	componment::{
		scrollbar::{self, Orientation},
		scrolllist::ScrollableList
	},
	fileinfo::FileInfo
};

#[derive(Debug)]
pub enum FinderIn {
	Up,
	Down,
	Clear,
	Refresh,
	ContentsExtend(Vec<FileInfo>),
	Query(String)
}

#[derive(Debug)]
pub enum FinderOut {
	FilterResult(String, Vec<(usize, Option<Vec<usize>>)>),
	Selected(Option<(usize, String)>),
	TotalCount(usize)
}

pub struct Finder {
	out_tx: Sender<FinderOut>,
	theme: SharedTheme,
	rect: Rect,
	query: String,
	contents: Arc<RwLock<Vec<FileInfo>>>,
	show_start: usize,
	selection: Option<usize>,
	filtered: Vec<(usize, Option<Vec<usize>>)>,
	filter_worker: FilterWorker
}

struct FilterWorkerMsg {
	query: String,
	contents: Arc<RwLock<Vec<FileInfo>>>,
	out_tx: Sender<FinderOut>
}

pub trait FinderItem: 'static {
	fn line(&self);
}

#[derive(Default)]
struct FilterWorker {
	filter_task_handler: Option<Arc<AtomicBool>>
}

impl FilterWorker {
	fn filter_start(&mut self, msg: FilterWorkerMsg) {
		let handler = Arc::new(AtomicBool::new(false));
		if let Some(task_handler) = &self.filter_task_handler.replace(handler.clone()) {
			if !task_handler.load(Ordering::Relaxed) {
				task_handler.swap(true, Ordering::Relaxed);
			}
		}

		let query = msg.query.clone();
		let content = msg.contents.clone();
		let sender = msg.out_tx.clone();

		thread::spawn(move || {
			let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
			macro_rules! maybe_stop {
				() => {
					if handler.load(Ordering::Relaxed) {
						return;
					}
				};
			}

			if query.is_empty() {
				let fr = content
					.read()
					.unwrap()
					.iter()
					.enumerate()
					.map(|entry| (entry.0, None))
					.collect::<Vec<(usize, Option<Vec<usize>>)>>();

				maybe_stop!();

				sender
					.send(FinderOut::FilterResult(query.clone(), fr))
					.map_err(|err| error!("unable to send content extend msg: {}", err))
					.ok();
			} else {
				let fr = content
					.read()
					.unwrap()
					.par_iter()
					.enumerate()
					.filter_map(|(i, s)| {
						matcher
							.fuzzy_indices(s.line(), query.as_ref())
							.map(|(score, indices)| (score, i, indices))
					})
					.map(|e| (e.1, Some(e.2.clone())))
					.collect::<Vec<(usize, Option<Vec<usize>>)>>();
				maybe_stop!();

				sender
					.send(FinderOut::FilterResult(query.clone(), fr))
					.map_err(|err| error!("unable to send content extend msg: {}", err))
					.ok();
			}
		});
	}
}

impl Finder {
	pub fn new(theme: SharedTheme, rect: Rect, out_tx: Sender<FinderOut>) -> Finder {
		Self {
			out_tx,
			query: "".to_string(),
			contents: Arc::new(RwLock::new(vec![])),
			selection: Some(0),
			filtered: vec![],
			theme,
			show_start: 0,
			rect,
			filter_worker: Default::default()
		}
	}

	pub fn update_filter(&mut self, query: String, filter: Vec<(usize, Option<Vec<usize>>)>) {
		if query == self.query {
			self.filtered = filter;
		}
	}

	fn filter_start(&mut self) {
		self.filter_worker.filter_start(FilterWorkerMsg {
			query: self.query.clone(),
			contents: self.contents.clone(),
			out_tx: self.out_tx.clone()
		});
	}

	fn update_query(&mut self, query: &str) {
		self.query = query.to_string();

		self.filter_start();

		self.selection = Some(0);
		self.show_start = 0;
	}

	fn move_selection(&mut self, move_type: FinderIn) -> bool {
		let new_selection = match move_type {
			FinderIn::Up => self.selection.map(|e| e.saturating_sub(1)),
			FinderIn::Down => self.selection.map(|e| e.saturating_add(1)),
			_ => {
				return false;
			}
		}
		.unwrap_or(usize::MAX);

		let new_selection = new_selection.clamp(0, self.filtered.len().saturating_sub(1));

		if new_selection != self.selection.unwrap_or(usize::MAX) {
			self.selection = Some(new_selection);

			match move_type {
				FinderIn::Up =>
					if self.selection.unwrap_or(0) <= self.show_start {
						self.show_start = self.show_start.saturating_sub(1);
					},
				FinderIn::Down =>
					if self.selection.unwrap_or(0) >= self.show_start + self.rect.height as usize {
						self.show_start = self.show_start.saturating_add(1);
					},
				_ => {}
			}
		}
		true
	}
}

impl Component for Finder {
	type MsgIn = FinderIn;

	fn draw(&self, f: &mut Frame) -> chin_tools::wrapper::anyhow::RResult<()> {
		let widget = self.widget();
		f.render_widget(widget, self.rect.clone());
		let list_height = self.rect.height as usize;

		if self.filtered.len() > list_height {
			scrollbar::draw_scrollbar(
				f,
				self.rect.clone(),
				self.filtered.len().saturating_sub(1),
				self.selection.unwrap_or(0),
				Orientation::Vertical
			);
		}

		Ok(())
	}

	fn widget(&self) -> impl ratatui::prelude::Widget {
		let area = self.rect.clone();

		let height = usize::from(area.height);
		let width = usize::from(area.width);

		let scroll_skip = self.show_start;

		let items = self
			.filtered
			.iter()
			.enumerate()
			.skip(scroll_skip)
			.take(height)
			.map(move |(id, (idx, indices))| {
				let selected = self.selection.map_or(false, |index| index == id);

				let binding = self.contents.read().unwrap();

				let full_text =
					chin_tools::utils::stringutils::trim_length_left(&binding[*idx].line(), width);
				let trim_length = self.contents.read().unwrap()[*idx]
					.line()
					.graphemes(true)
					.count() - full_text.graphemes(true).count();
				Line::from(
					full_text
						.graphemes(true)
						.enumerate()
						.map(|(c_idx, c)| {
							Span::styled(
								Cow::from(c.to_string()),
								self.theme.text(
									indices
										.as_ref()
										.map_or(false, |e| e.contains(&(c_idx + trim_length))),
									selected
								)
							)
						})
						.collect::<Vec<_>>()
				)
			});

		ScrollableList::new(items).block(Block::default().borders(Borders::RIGHT))
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
					crossterm::event::KeyCode::Up => {
						self.move_selection(FinderIn::Up);
						(NeedRedraw::Yes, ConsumeState::Consumed)
					}
					crossterm::event::KeyCode::Down => {
						self.move_selection(FinderIn::Down);
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
			FinderIn::Clear => {
				self.contents.write().unwrap().clear();
				self.filtered.clear();
			}
			FinderIn::Refresh => {}
			FinderIn::ContentsExtend(adds) => {
				self.contents.write().unwrap().extend(adds);
				let query = self.query.to_string();
				self.update_query(query.as_str());
				self.out_tx
					.send(FinderOut::TotalCount(self.contents.read().unwrap().len()))
					.unwrap();
			}
			FinderIn::Query(query) => self.update_query(query.as_str()),
			_ => {}
		}
	}
}
