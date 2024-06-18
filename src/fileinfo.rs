use std::{
	fs::Metadata,
	path::{Path, PathBuf}
};

use magic::Cookie;
use tracing::warn;

#[derive(Clone, Debug)]
pub struct FileInfo {
	pathbuf: PathBuf,
	pathstr: String,
	show_start: usize,
	pub desc: Option<String>,
	pub metadata: Option<Metadata>
}

impl PartialEq for FileInfo {
	fn eq(&self, other: &Self) -> bool {
		self.pathbuf == other.pathbuf
	}
}

fn diff_path(base: &str, total: &str) -> usize {
	if base == "/" {
		return 0;
	}

	if !total.starts_with(base) {
		warn!("{} should be prefix of {}", base, total);
		0
	} else {
		if base.ends_with("/") {
			base.len()
		} else {
			base.len() + 1
		}
	}
}

impl FileInfo {
	pub fn new(path: PathBuf, base: &str, metadata: Option<Metadata>) -> Self {
		let pathstr = path.as_os_str().to_string_lossy().to_string();
		let show_start = diff_path(base, &pathstr);
		Self {
			pathstr,
			pathbuf: path,
			show_start,
			desc: None,
			metadata
		}
	}

	pub fn set_file_info(&mut self, cookie: Option<&Cookie<magic::cookie::Load>>) {
		if self.desc.is_some() {
			return;
		}

		if let Some(cookie) = cookie {
			self.desc = cookie.file(self.pathstr.as_str()).ok();
		}
	}

	pub fn line(&self) -> &str {
		&self.pathstr[self.show_start..]
	}
}
