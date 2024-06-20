/* use std::process::Command;

use super::Viewer;

pub struct ImageViewer {}

impl ImageViewer {
	pub fn new() -> Self {
		Self {}
	}
}

impl Viewer for ImageViewer {
	async fn handle_fileinfo<F>(
		&self,
		fileinfo: crate::fileinfo::FileInfo,
		cancel_signal: F
	) -> Result<super::ViewMsg, super::FileInfoHandleErr>
	where
		F: Fn() -> bool + Clone
	{
		let watch_file = "/tmp/ffp-wtach";
		let filepath = fileinfo.path().as_os_str().to_string_lossy();
		tokio::fs::write(watch_file, format!("{}\n", filepath))
			.await
			.map_err(|e| Err(super::FileInfoHandleErr::Error(e.to_string())));

		Command::new("echo")
			.arg(filepath.to_string())
			.env("FFP_WATCH_FILE", watch_file)
			.spawn()
			.map_err(|e| Err(super::FileInfoHandleErr::Error(e.to_string())));

		Ok()
	}

	fn handle_event(&mut self, event: crossterm::event::Event) {
		todo!()
	}

	fn draw(
		&self,
		view_msg: &super::ViewMsg,
		show_cursor: u16,
		f: &mut ratatui::Frame,
		rect: &ratatui::prelude::Rect
	) {
		todo!()
	}
}
 */