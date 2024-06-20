use std::io::{stdout, Stdout};

use chin_tools::wrapper::anyhow::RResult;
use crossterm::{
	event::{Event, KeyCode, KeyModifiers},
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
	app::{
		finder::{Finder, FinderIn},
		input::Input,
		preview::FileViewer,
		status::Status,
		theme::{SharedTheme, Theme},
		Component
	},
	dirwalker::{self, DirFilter},
	fileinfo::FileInfo
};

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Areas {
	pub finder: Rect,
	pub status: Rect,
	pub input: Rect,
	pub stage: Rect
}

bitflags::bitflags! {
	#[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
	pub struct ComponentEnum: u8 {
		const FINDER = 0b0000_0001;
		const STATUS = 0b0000_0010;
		const INPUT = 0b0000_0100;
		const STAGE = 0b0000_1000;
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
				Constraint::Min(1),
				Constraint::Length(1),
				Constraint::Length(1)
			])
			.horizontal_margin(0)
			.split(left_panel);

		Areas {
			finder: ls[0],
			status: ls[1],
			input: ls[2],
			stage: right_panel
		}
	}

	pub async fn run(&mut self, term: &mut Terminal<CrosstermBackend<Stdout>>) -> RResult<()> {
		let (input_out_tx, input_out_rx) = flume::unbounded();
		let mut input_out_rx = input_out_rx.stream();

		let (finder_in_tx, finder_in_rx) = flume::unbounded::<FinderIn>();
		let mut finder_in_rx = finder_in_rx.stream();

		let (finder_out_tx, finder_out_rx) = flume::unbounded();
		let mut finder_out_rx = finder_out_rx.stream();

		let (stage_out_tx, stage_out_rx) = flume::unbounded();
		let mut stage_out_rx = stage_out_rx.stream();

		let mut ev_stream = crossterm::event::EventStream::new();

		let cwd = self.initial_wd.as_str();
		dirwalker::rebuild_dirlist_start(finder_in_tx.clone(), cwd, DirFilter::builder().build());

		let mut input = Input::new(input_out_tx);
		let mut finder = Finder::new(self.theme.clone(), finder_out_tx);
		let mut status = Status::new(cwd);
		let mut viewer = FileViewer::new(stage_out_tx);

		let mut changed_coms = ComponentEnum::all();

		Ok(loop {
			let frame = term.get_frame();
			let areas = Tui::layout(&frame);

			if !changed_coms.is_empty() {
				term.draw(|f| {
					let render_result: RResult<()> = (|| {
						execute!(stdout(), BeginSynchronizedUpdate)?;
						input.draw(f, &areas.input, changed_coms.contains(ComponentEnum::INPUT))?;

						finder.draw(
							f,
							&areas.finder,
							changed_coms.contains(ComponentEnum::FINDER)
						)?;

						status.draw(
							f,
							&areas.status,
							changed_coms.contains(ComponentEnum::STATUS)
						)?;
						viewer.draw(
							f,
							&areas.stage,
							changed_coms.contains(ComponentEnum::STAGE)
						)?;
						Ok(())
					})();

					match render_result {
						Ok(_) => {}
						Err(err) => {
							tracing::error!("unable to render {}", err);
						}
					}
				})?;

				execute!(stdout(), EndSynchronizedUpdate)?;
			}

			changed_coms = tokio::select! {
				Some(ev) = ev_stream.next().fuse() => {
					let mut redraw = ComponentEnum::empty();
					if let Ok(ev) = ev {
						if let Event::Key(key) = &ev {
							if key.code == crossterm::event::KeyCode::Esc ||
							(key.modifiers.contains(KeyModifiers::CONTROL) && match key.code {
								KeyCode::Char('c') => true,
								_ => false,
							}) {
								break
							}
						}

						let mut res = input.handle_event(&ev);
						if res.redraw {
							redraw |= ComponentEnum::INPUT
						}

						if !res.consumed {
							res = finder.handle_event(&ev);
							if res.redraw {
								redraw |= ComponentEnum::FINDER
							}
						}

						if !res.consumed {
							res = viewer.handle_event(&ev);
							if res.redraw {
								redraw |= ComponentEnum::FINDER
							}
						}
					}

					redraw
				},
				Some(ev) = input_out_rx.next() => {
					match ev {
						crate::app::input::InputOut::Input(input) => {
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
						crate::app::finder::FinderOut::FilterResult(query, fr) => {
							status.set_filter_count(fr.len());
							finder.update_filter(query, fr);

							ComponentEnum::FINDER | ComponentEnum::STATUS
						},
						crate::app::finder::FinderOut::Selected(selected) => {
							viewer.handle_file(selected.as_ref());
							self.cur_file = selected.map(|e| e.into());

							ComponentEnum::STAGE
						},
						crate::app::finder::FinderOut::TotalCount(count) => {
							status.set_total(count);
							ComponentEnum::STATUS
						},
					}
				},
				Some(ev) = stage_out_rx.next() => {
					viewer.set_view(ev);

					ComponentEnum::STAGE
				}
			};
		})
	}
}
