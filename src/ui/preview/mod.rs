use crossterm::event::Event;

use crate::fileinfo::FileInfo;

pub mod attr;
pub mod text;

pub struct FileViewer {
	view_type: ViewType,
	file_info: FileInfo
}

pub trait Viewer {
	fn reset(&mut self);
	fn set_fileinfo(&mut self, fileinfo: &FileInfo);
	fn handle_event(&mut self, event: Event);
}

pub enum ViewType {
	Text
}
