use std::{
	fs::Metadata,
	path::{Path, PathBuf}
};

use tracing::warn;

#[derive(Clone, Debug)]
pub struct FilePath {
	pub pathbuf: PathBuf,
	pathstr: String,
	show_start: usize
}

impl Into<FileInfo> for FilePath {
	fn into(self) -> FileInfo {
		FileInfo {
			path: self,
			desc: None,
			metadata: Err("empty".to_string())
		}
	}
}

#[derive(Clone, Debug)]
pub struct FileInfo {
	pub path: FilePath,
	pub desc: Option<String>,
	pub metadata: Result<Metadata, String>
}

impl PartialEq for FilePath {
	fn eq(&self, other: &Self) -> bool {
		self.pathbuf == other.pathbuf
	}
}

impl PartialEq for FileInfo {
	fn eq(&self, other: &Self) -> bool {
		self.path == other.path
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

impl FilePath {
	pub fn new(pathbuf: PathBuf, base: &str) -> Self {
		let pathstr = pathbuf.as_os_str().to_string_lossy().to_string();
		let show_start = diff_path(base, &pathstr);
		FilePath {
			pathstr,
			pathbuf,
			show_start
		}
	}

	pub fn path(&self) -> &Path {
		&self.pathbuf
	}

	pub fn line(&self) -> &str {
		&self.pathstr[self.show_start..]
	}
}

impl FileInfo {
	pub fn path(&self) -> &Path {
		&self.path.path()
	}
}
