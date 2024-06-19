use std::{
	borrow::Cow,
	cell::RefCell,
	rc::Rc,
	sync::{
		atomic::{AtomicU64, Ordering},
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
use tracing::{error, info};
use unicode_segmentation::UnicodeSegmentation;

use super::{theme::SharedTheme, Component, ConsumeP, RedrawP};
use crate::{
	componment::{
		scrollbar::{self, Orientation},
		scrolllist::ScrollableList
	},
	fileinfo::FilePath
};

#[derive(Debug)]
pub enum FinderIn {
	Clear,
	Refresh,
	ContentsExtend(Vec<FilePath>),
	Query(String)
}

#[derive(Debug, Clone)]
pub enum FinderMove {
	Up,
	Down,
	Nil
}

#[derive(Debug)]
pub enum FinderOut {
	FilterResult(String, FileterResultEnum),
	Selected(FilePath),
	TotalCount(usize)
}

#[derive(Debug)]
pub enum FileterResultEnum {
	All(usize),
	Vec(Arc<Vec<usize>>),
	None
}

impl From<Vec<usize>> for FileterResultEnum {
	fn from(value: Vec<usize>) -> Self {
		Self::Vec(Arc::new(value))
	}
}

impl FileterResultEnum {
	pub fn len(&self) -> usize {
		match self {
			FileterResultEnum::All(count) => *count,
			FileterResultEnum::Vec(vec) => vec.len(),
			FileterResultEnum::None => 0
		}
	}
}

pub struct Finder {
	out_tx: Sender<FinderOut>,
	theme: SharedTheme,
	selection: Option<usize>,
	show_start: usize,
	last_move: FinderMove,

	cached_selection: Rc<RefCell<Option<FilePath>>>,

	contents: Arc<RwLock<Vec<FilePath>>>,
	query: String,
	filtered: FileterResultEnum,
	filter_worker: FilterWorker
}

struct FilterWorkerMsg {
	query: String,
	contents: Arc<RwLock<Vec<FilePath>>>,
	out_tx: Sender<FinderOut>
}

pub trait FinderItem: 'static {
	fn line(&self);
}

#[derive(Default)]
struct FilterWorker {
	filter_task_handler: Arc<AtomicU64>,
	filter_result: Option<Arc<(String, Vec<usize>)>>
}

impl FilterWorker {
	fn filter_start(&mut self, msg: FilterWorkerMsg) {
		let handler = self.filter_task_handler.clone();
		handler.fetch_add(1, Ordering::Relaxed);

		let query = msg.query.clone();
		let content = msg.contents.clone();
		let sender = msg.out_tx.clone();
		let filtered = self.filter_result.clone();

		thread::spawn(move || {
			let ticket = handler.load(Ordering::Relaxed);
			let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
			macro_rules! maybe_stop {
				() => {
					if handler.load(Ordering::Relaxed) != ticket {
						return;
					}
				};
			}

			macro_rules! maybe_stop2 {
				() => {
					if handler.load(Ordering::Relaxed) != ticket {
						return None;
					}
				};
			}

			if query.is_empty() {
				maybe_stop!();

				sender
					.send(FinderOut::FilterResult(
						query.clone(),
						FileterResultEnum::All(content.read().unwrap().len())
					))
					.map_err(|err| error!("unable to send content extend msg: {}", err))
					.ok();
			} else {
				let content = content.read().unwrap();

				let fr = if filtered.is_some()
					&& query.contains(filtered.as_ref().unwrap().0.as_str())
				{
					filtered
						.as_ref()
						.unwrap()
						.1
						.par_iter()
						.filter_map(|s| {
							maybe_stop2!();
							if let Some(line) = content.get(*s) {
								matcher
									.fuzzy_indices(line.line(), query.as_ref())
									.map(|(score, indices)| (score, *s, indices))
							} else {
								None
							}
						})
						.map(|e| e.1)
						.collect::<Vec<usize>>()
				} else {
					content
						.par_iter()
						.enumerate()
						.filter_map(|(i, s)| {
							maybe_stop2!();
							matcher
								.fuzzy_indices(s.line(), query.as_ref())
								.map(|(score, indices)| (score, i, indices))
						})
						.map(|e| e.1)
						.collect::<Vec<usize>>()
				};

				maybe_stop!();

				sender
					.send(FinderOut::FilterResult(query.clone(), fr.into()))
					.map_err(|err| error!("unable to send content extend msg: {}", err))
					.ok();
			}
		});
	}
}
impl Finder {
	pub fn new(theme: SharedTheme, out_tx: Sender<FinderOut>) -> Finder {
		Self {
			out_tx,
			query: "".to_string(),
			contents: Arc::new(RwLock::new(vec![])),
			selection: Some(0),
			filtered: FileterResultEnum::All(0),
			theme,
			show_start: 0,
			filter_worker: Default::default(),
			cached_selection: Default::default(),
			last_move: FinderMove::Nil
		}
	}

	pub fn update_filter(&mut self, query: String, filter: FileterResultEnum) {
		if query == self.query {
			self.filtered = filter;
		}
	}

	fn filtered_len(&self) -> usize {
		match &self.filtered {
			FileterResultEnum::All(c) => *c,
			FileterResultEnum::Vec(vec) => vec.len(),
			FileterResultEnum::None => 0
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
	}

	fn move_selection(&mut self, move_type: FinderMove) -> bool {
		self.last_move = move_type.clone();
		let new_selection = match move_type {
			FinderMove::Up => self.selection.map(|e| e.saturating_sub(1)),
			FinderMove::Down => self.selection.map(|e| e.saturating_add(1)),
			_ => {
				return false;
			}
		}
		.unwrap_or(usize::MAX);

		let new_selection = new_selection.clamp(0, self.filtered_len().saturating_sub(1));

		if self.selection.map_or(true, |e| new_selection != e) {
			self.selection = Some(new_selection);
		}

		true
	}
}

impl Component for Finder {
	type MsgIn = FinderIn;

	fn draw(
		&mut self,
		f: &mut Frame,
		rect: &Rect,
		changed: bool
	) -> chin_tools::wrapper::anyhow::RResult<()> {
		let list_height = rect.height as usize;
		let selection_num = self.selection.unwrap_or(0);

		if selection_num == 0 {
			self.selection = Some(0);
			self.show_start = 0;
		}

			match self.last_move {
				FinderMove::Up =>
					if selection_num <= self.show_start + 3 {
						self.show_start = self.show_start.saturating_sub(1);
					},
				FinderMove::Down =>
					if selection_num >= self.show_start + rect.height as usize - 3 {
						self.show_start = self.show_start.saturating_add(1);
					},
				_ => {}
			}

		let widget = self._widget(rect, changed);
		f.render_widget(widget, rect.clone());

		if self.filtered_len() > list_height {
			scrollbar::draw_scrollbar(
				f,
				rect.clone(),
				self.filtered_len().saturating_sub(1),
				self.selection.unwrap_or(0),
				Orientation::Vertical
			);
		}

		Ok(())
	}

	fn _widget(&self, rect: &Rect, _changed: bool) -> impl ratatui::prelude::Widget {
		let height = usize::from(rect.height);

		let scroll_skip = self.show_start;
		let selection = self.selection;
		let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();

		let page: Vec<(usize, usize)> = match &self.filtered {
			FileterResultEnum::All(_) => self
				.contents
				.read()
				.unwrap()
				.iter()
				.enumerate()
				.skip(scroll_skip)
				.take(height)
				.map(|(id, _)| (id, id))
				.collect(),
			FileterResultEnum::Vec(vec) => vec
				.clone()
				.iter()
				.enumerate()
				.skip(scroll_skip)
				.take(height)
				.map(|(id, idx)| (id, *idx))
				.collect(),
			FileterResultEnum::None => vec![]
		};

		let items = page
			.iter()
			.map(move |(id, idx)| {
				let selected = selection.map_or(false, |index| index == *id);
				let vec = self.contents.read().unwrap();
				if selected {
					let selected = vec.get(*idx).map(|e| e.clone());
					if let Some(selected) = selected {
						let mut cached = self.cached_selection.borrow_mut();
						let ofi = cached.as_ref();
						if ofi.map_or(true, |fi| *fi != selected) {
							self.out_tx
								.send(FinderOut::Selected(selected.clone()))
								.unwrap();
							cached.replace(selected.clone());
						}
					}
				}

				let line = vec[*idx].line();
				let full_text = line;
				let trim_length = line.graphemes(true).count() - full_text.graphemes(true).count();

				let indices = matcher
					.fuzzy_indices(line, &self.query)
					.map(|(_, indices)| indices);
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
			})
			.collect::<Vec<Line>>();

		ScrollableList::new(items.into_iter()).block(Block::default().borders(Borders::RIGHT))
	}

	fn handle_event(&mut self, event: Event) -> (RedrawP, ConsumeP) {
		match event {
			Event::Key(key) => {
				/* 				if key.modifiers != KeyModifiers::NONE || key.modifiers != KeyModifiers::SHIFT {
					return false;
				} */

				match key.code {
					crossterm::event::KeyCode::Up => {
						self.move_selection(FinderMove::Up);
						(RedrawP::Yes, ConsumeP::Yes)
					}
					crossterm::event::KeyCode::Down => {
						self.move_selection(FinderMove::Down);
						(RedrawP::Yes, ConsumeP::Yes)
					}
					_ => (RedrawP::No, ConsumeP::No)
				}
			}
			_ => (RedrawP::No, ConsumeP::No)
		}
	}

	fn handle_msg(&mut self, msg: Self::MsgIn) {
		match msg {
			FinderIn::Clear => {
				self.contents.write().unwrap().clear();
				self.filtered = FileterResultEnum::None;
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
			FinderIn::Query(query) => self.update_query(query.as_str())
		}
	}
}
