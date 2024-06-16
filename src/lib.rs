use std::sync::Arc;

use dirwalker::FindType;

pub mod componment;
pub mod constant;
pub mod dirwalker;
pub mod fileinfo;
pub mod tui;
pub mod ui;

pub struct AppState {
	pub option: Arc<String>,
	pub show_mode: FindType
}
