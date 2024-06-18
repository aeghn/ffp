use std::path::PathBuf;

use ratatui::widgets::Widget;

use crate::fileinfo::FileInfo;

pub mod text;

pub struct FileView {
	view_type: ViewType,
	file_info: FileInfo
}

pub enum ViewType {
	Text
}
