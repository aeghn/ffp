use std::io::{stdout, Stdout};

use chin_tools::wrapper::anyhow::RResult;
use crossterm::{
	event::Event,
	execute,
	terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate}
};
use futures_util::{FutureExt, StreamExt};
use ratatui::{
	backend::CrosstermBackend,
	layout::{Constraint, Layout, Rect},
	Frame, Terminal
};

use crate::{
	dirwalker::{self, DirFilter},
	fileinfo::FileInfo,
	ui::{
		attr::FileAttr,
		finder::{Finder, FinderIn},
		input::Input,
		status::Status,
		theme::{SharedTheme, Theme},
		Component, RedrawP
	}
};

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Areas {
	pub finder: Rect,
	pub status: Rect,
	pub input: Rect,
	pub info: Rect,
	pub stage: Rect
}

bitflags::bitflags! {
	#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
	pub struct ComponentEnum: u8 {
		const FINDER = 0b0000_0001;
		const STATUS = 0b0000_0010;
		const INPUT = 0b0000_0100;
		const INFO = 0b0000_1000;
		const STAGE = 0b0001_0000;
	}
}

pub struct Tui {
	theme: SharedTheme,
	initial_wd: String,
	cur_file: Option<FileInfo>
}

impl Tui {
	pub fn new(initial_wd: &str) -> Self {
		let theme = SharedTheme::new(Theme::default());
		Tui {
			theme,
			initial_wd: initial_wd.to_string(),
			cur_file: None
		}
	}

	pub fn layout(frame: &Frame<'_>) -> Areas {
		let lr = Layout::default()
			.direction(ratatui::layout::Direction::Horizontal)
			.constraints([Constraint::Fill(1), Constraint::Fill(1)])
			.horizontal_margin(1)
			.vertical_margin(1)
			.split(frame.size());

		let left_panel = lr[0];
		let right_panel = lr[1];

		let ls = Layout::default()
			.constraints([
				Constraint::Length(1),
				Constraint::Length(1),
				Constraint::Min(1)
			])
			.horizontal_margin(0)
			.split(left_panel);
		let rs = Layout::default()
			.constraints([Constraint::Min(1), Constraint::Length(2)])
			.horizontal_margin(0)
			.split(right_panel);

		Areas {
			finder: ls[2],
			status: ls[1],
			input: ls[0],
			info: rs[1],
			stage: rs[0]
		}
	}

	pub async fn run(&mut self, term: &mut Terminal<CrosstermBackend<Stdout>>) -> RResult<()> {
		let (input_out_tx, input_out_rx) = flume::unbounded();
		let mut input_out_rx = input_out_rx.stream();

		let (finder_in_tx, finder_in_rx) = flume::unbounded::<FinderIn>();
		let mut finder_in_rx = finder_in_rx.stream();

		let (finder_out_tx, finder_out_rx) = flume::unbounded();
		let mut finder_out_rx = finder_out_rx.stream();

		let mut ev_stream = crossterm::event::EventStream::new();

		let cwd = self.initial_wd.as_str();
		dirwalker::rebuild_dirlist_start(finder_in_tx.clone(), cwd, DirFilter::builder().build());

		let mut input = Input::new(input_out_tx);
		let mut finder = Finder::new(self.theme.clone(), finder_out_tx);
		let mut status = Status::new(cwd);

		let mut changed_coms = ComponentEnum::all();

		let cookie = magic::Cookie::open(magic::cookie::Flags::ERROR)?;

		let database = Default::default();

		let cookie = cookie.load(&database).ok();

		Ok(loop {
			let frame = term.get_frame();
			let areas = Tui::layout(&frame);

			term.draw(|f| {
				execute!(stdout(), BeginSynchronizedUpdate).unwrap();
				input
					.draw(f, &areas.input, changed_coms.contains(ComponentEnum::INPUT))
					.unwrap();

				finder
					.draw(
						f,
						&areas.finder,
						changed_coms.contains(ComponentEnum::FINDER)
					)
					.unwrap();

				status
					.draw(
						f,
						&areas.status,
						changed_coms.contains(ComponentEnum::STATUS)
					)
					.unwrap();

				if let Some(file) = self.cur_file.as_mut() {
					file.set_file_info(cookie.as_ref());
					let mut fs = FileAttr::new(
						file.metadata.as_ref(),
						file.desc.as_ref(),
						areas.info.clone()
					);
					fs.draw(f, &areas.info, changed_coms.contains(ComponentEnum::INFO))
						.unwrap();
				}
			})?;

			execute!(stdout(), EndSynchronizedUpdate)?;

			changed_coms = tokio::select! {
				Some(ev) = ev_stream.next().fuse() => {
					tracing::trace!("msg: ev stream");
					let redraw = ComponentEnum::empty();
					if let Ok(ev) = ev {
						if let Event::Key(key) = ev.clone() {
							if key.code == crossterm::event::KeyCode::Esc {
								break
							}
						}

						let res = input.handle_event(ev.clone());
						if res.1.yes() {
							if RedrawP::Yes == res.0 {
								changed_coms = ComponentEnum::INPUT
							}
						}

						if !res.1.yes() {
							let res = finder.handle_event(ev.clone());
							if res.0.yes() {
								changed_coms |= ComponentEnum::FINDER
							}
						}
					}

					redraw
				},
				Some(ev) = input_out_rx.next() => {
					match ev {
						crate::ui::input::InputOut::Input(input) => {
							finder_in_tx.send(FinderIn::Query(input.clone()))
							.map_err(|err| {
								tracing::error!("unable to send Query msg: {}", err)
							})
							.ok();
							ComponentEnum::INPUT
						},
					}
				},
				Some(ev) = finder_in_rx.next() => {
					tracing::trace!("msg: finder in rx");
					finder.handle_msg(ev);
					ComponentEnum::empty()
				},
				Some(ev) = finder_out_rx.next() => {
					tracing::trace!("msg: finder out rx");
					match ev {
						crate::ui::finder::FinderOut::FilterResult(query, fr) => {
							status.set_filter_count(fr.len());
							finder.update_filter(query, fr);

							ComponentEnum::FINDER | ComponentEnum::STATUS
						},
						crate::ui::finder::FinderOut::Selected(selected) => {
							self.cur_file.replace(selected.clone());
							ComponentEnum::INFO
						},
						crate::ui::finder::FinderOut::TotalCount(count) => {
							status.set_total(count);
							ComponentEnum::STATUS
						},
					}
				}
			};
		})
	}
}
