use std::fs::Metadata;

use file_format::FileFormat;
use magic::Cookie;
use tracing::warn;

#[derive(Clone, Debug)]
pub struct FileInfo {
	pub path: String,
	show_start: usize,
	pub desc: Option<String>,
	pub metadata: Option<Metadata>
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
	pub fn new(path: &str, base: &str, metadata: Option<Metadata>) -> Self {
		Self {
			path: path.to_string(),
			show_start: diff_path(base, path),
			desc: None,
			metadata
		}
	}

	pub fn set_file_info(&mut self, cookie: Option<&Cookie<magic::cookie::Load>>) {
		if self.desc.is_some() {
			return;
		}

		if let Some(cookie) = cookie {
			self.desc = cookie.file(self.path.as_str()).ok();
		}
	}

	pub fn line(&self) -> &str {
		&self.path[self.show_start..]
	}
}
