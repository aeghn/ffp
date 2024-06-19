#![feature(if_let_guard)]

use std::env;

use crossterm::{
	execute,
	terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}
};
use ffp::tui::Tui;
use ratatui::{backend::CrosstermBackend, Terminal};
use ratatui_image::picker::Picker;
use tracing::error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let file_appender = tracing_appender::rolling::daily("/tmp/", "ffp.log");
	let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
	tracing_subscriber::fmt().with_writer(non_blocking).init();

	let window_size = chin_tools::utils::termutils::get_window_size_px()?;

	// We need to create `picker` on this thread because if we create it on the `renderer` thread,
	// it messes up something with user input. Input never makes it to the crossterm thing
	let mut picker = Picker::new((
		window_size.width / window_size.columns,
		window_size.height / window_size.rows
	));
	picker.guess_protocol();

	// then we want to spawn off the rendering task
	// We need to use the thread::spawn API so that this exists in a thread not owned by tokio,
	// since the methods we call in `start_rendering` will panic if called in an async context
	std::thread::spawn(move || {
		// renderer::start_rendering(file_path, render_tx, render_rx, window_size)
	});

	let backend = CrosstermBackend::new(std::io::stdout());
	let mut term = Terminal::new(backend)?;

	execute!(
		term.backend_mut(),
		EnterAlternateScreen,
		crossterm::cursor::Hide
	)?;
	enable_raw_mode()?;

	let mut tui = Tui::new(env::current_dir()?.to_string_lossy().to_string().as_str());

	match tui.run(&mut term).await {
		Ok(_) => {}
		Err(err) => {
			error!("Some error occurs handling the tui event: {}", err);
		}
	}

	execute!(
		term.backend_mut(),
		LeaveAlternateScreen,
		crossterm::cursor::Show
	)?;
	disable_raw_mode()?;

	Ok(())
}
