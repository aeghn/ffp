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
		fileinfo::FileSkim,
		finder::{Finder, FinderIn},
		input::Input,
		status::Status,
		theme::{SharedTheme, Theme},
		Component, ConsumeState, NeedRedraw
	}
};

pub struct Tui {
	initial_wd: String
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Areas {
	pub finder: Rect,
	pub status: Rect,
	pub input: Rect,
	pub info: Rect,
	pub stage: Rect
}

impl Tui {
	pub fn new(initial_wd: &str) -> Self {
		Tui {
			initial_wd: initial_wd.to_string()
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
			.split(left_panel);
		let rs = Layout::default()
			.constraints([Constraint::Length(4), Constraint::Min(1)])
			.split(right_panel);

		Areas {
			finder: ls[2],
			status: ls[1],
			input: ls[0],
			info: rs[0],
			stage: rs[1]
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

		let frame = term.get_frame();
		let areas = Tui::layout(&frame);
		let theme = SharedTheme::new(Theme::default());

		let cwd = self.initial_wd.as_str();
		dirwalker::rebuild_dirlist_start(finder_in_tx.clone(), cwd, DirFilter::builder().build());

		let mut input = Input::new(areas.input.clone(), input_out_tx);
		let mut finder = Finder::new(theme.clone(), areas.finder.clone(), finder_out_tx);
		let mut status = Status::new(cwd, areas.status.clone());
		let mut fileinfo: Option<FileInfo> = None;

		let mut needs_redraw = true;

		let cookie = magic::Cookie::open(magic::cookie::Flags::ERROR)?;

		let database = Default::default();

		let cookie = cookie.load(&database).ok();

		Ok(loop {
			if needs_redraw {
				term.draw(|f| {
					execute!(stdout(), BeginSynchronizedUpdate).unwrap();
					input.draw(f).unwrap();
					finder.draw(f).unwrap();
					status.draw(f).unwrap();
/* 					if let Some(file) = fileinfo.as_mut() {
						// file.set_file_info(cookie.as_ref());
						let fs = FileSkim::new(
							file.metadata.as_ref(),
							file.desc.as_ref(),
							areas.info.clone()
						);
						fs.draw(f).unwrap();
					} */
				})?;
			}

			execute!(stdout(), EndSynchronizedUpdate)?;

			needs_redraw = tokio::select! {
				Some(ev) = ev_stream.next().fuse() => {
					let mut redraw = false;
					let mut handled = false;
					if let Ok(ev) = ev {
						if let Event::Key(key) = ev.clone() {
							if key.code == crossterm::event::KeyCode::Esc {
								break
							}
						}



						let res = input.handle_event(ev.clone());
						if res.1 == ConsumeState::Consumed {
							redraw = NeedRedraw::Yes == res.0 || redraw;
						}

						if res.1 != ConsumeState::Consumed {
							let res = finder.handle_event(ev.clone());
							redraw = NeedRedraw::Yes == res.0 || redraw;
							handled = res.1 == ConsumeState::Consumed;
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
							false
						},
					}
				},
				Some(ev) = finder_in_rx.next() => {
					finder.handle_msg(ev);
					true
				},
				Some(ev) = finder_out_rx.next() => {
					match ev {
						crate::ui::finder::FinderOut::FilterResult(query, fr) => {
							status.set_filter(fr.len());
							finder.update_filter(query, fr);

							true
						},
						crate::ui::finder::FinderOut::Selected(selected) => {
							fileinfo.replace(FileInfo::new(selected.path.as_str(), cwd, selected.metadata.clone()));
							true
						},
						crate::ui::finder::FinderOut::TotalCount(count) => {
							status.set_total(count);
							true
						},
					}
				}
			};
		})
	}
}
