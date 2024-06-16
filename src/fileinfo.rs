use std::fs::{Metadata};

use file_format::FileFormat;

#[derive(Clone, Debug)]
pub struct FileInfo {
	path: String,
	show_start: usize,
	desc: Option<FileFormat>,
	metadata: Option<Metadata>
}

impl FileInfo {
	pub fn new(path: &str, show_start: usize, metadata: Option<Metadata>) -> Self {
		Self {
			path: path.to_string(),
			show_start,
			desc: None,
			metadata
		}
	}

	pub fn set_file_info(&mut self) {
		let desc = file_format::FileFormat::from_file(&self.path).ok();
		self.desc = desc;
	}

	pub fn line(&self) -> &str {
		&self.path[self.show_start..]
	}
}
