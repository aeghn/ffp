use std::path::Path;

use chin_tools::wrapper::anyhow::RResult;
use flume::Sender;
use futures_util::StreamExt;
use tokio::{fs::File, io::AsyncReadExt};
use tracing::error;

use crate::{fileinfo::FilePath, ui::finder::FinderIn};

#[derive(Clone, Default)]
pub enum FindType {
	LS,
	#[default]
	FIND
}

#[derive(Clone, Default)]
pub struct DirFilter {
	find_type: FindType,
	dotfile: bool
}

pub struct DirFilterBuilder {
	filter: DirFilter
}

impl DirFilter {
	pub fn builder() -> DirFilterBuilder {
		DirFilterBuilder {
			filter: Default::default()
		}
	}
}

impl DirFilterBuilder {
	pub fn with_find_type(self, find_type: FindType) -> Self {
		DirFilterBuilder {
			filter: DirFilter {
				find_type,
				..self.filter
			}
		}
	}

	pub fn build(self) -> DirFilter {
		self.filter
	}
}

pub fn rebuild_dirlist_start(sender: Sender<FinderIn>, cwd: &str, filter: DirFilter) {
	let cwd = cwd.to_string();
	tokio::spawn(async move {
		let cwd_ref = cwd.as_str();
		walk_dir(sender, cwd_ref, filter).await
	});
}

pub async fn walk_dir(tx: Sender<FinderIn>, cwd: &str, filter: DirFilter) {
	let mut items: Vec<FilePath> = Vec::with_capacity(50000);
	if let Err(err) = tx.send_async(FinderIn::Clear).await {
		tracing::error!("unable to send clear msg, {}", err);
	}
	match filter.find_type {
		FindType::LS =>
			if let Ok(mut dir) = tokio::fs::read_dir(cwd).await {
				while let Ok(Some(en)) = dir.next_entry().await {
					let path = en.path();
					let info = FilePath::new(path, cwd);
					items.push(info);
					if items.len() > 50000 {
						tx.send_async(FinderIn::ContentsExtend(items))
							.await
							.map_err(|err| error!("unable to send content extend msg: {}", err))
							.ok();
						items = Vec::with_capacity(50000);
					}
				}
			},
		FindType::FIND => {
			let mut wd = async_walkdir::WalkDir::new(cwd);
			while let Some(en) = wd.next().await {
				match en {
					Ok(de) => {
						items.push(FilePath::new(de.path(), cwd));
						if items.len() > 50000 {
							tx.send_async(FinderIn::ContentsExtend(items))
								.await
								.map_err(|err| error!("unable to send content extend msg: {}", err))
								.ok();
							items = Vec::with_capacity(50000);
						}
					}
					Err(err) => {
						error!("unable to read file, err: {}", err)
					}
				}
			}
		}
	}

	tx.send_async(FinderIn::ContentsExtend(items))
		.await
		.map_err(|err| error!("unable to send content extend msg: {}", err))
		.ok();
}

pub async fn read_dir2<F>(cwd: &Path, cancel: &F) -> RResult<(Vec<String>, usize)>
where
	F: Fn() -> bool
{
	let mut dir = tokio::fs::read_dir(cwd).await?;
	let mut count = 0;
	let mut result = vec![];
	while let en = dir.next_entry().await {
		if cancel() {
			tracing::info!("cancelled");
			break;
		}

		match en {
			Ok(None) => {
				break;
			}
			Ok(Some(de)) => {
				result.push(de.file_name().to_string_lossy().to_string());
				count += 1;
			}
			Err(_) => {
				count += 1;
			}
		}
	}
	Ok((result, count))
}

pub async fn read_first_n_chars(path: &Path, n: usize) -> RResult<String> {
	let mut file = File::open(path).await?;

	let mut buffer = vec![0u8; n];

	file.read_buf(&mut buffer).await?;

	let result = String::from_utf8(buffer)?;

	Ok(result)
}

pub async fn file_is_text(path: &Path) -> RResult<bool> {
	if !path.is_file() {
		return Ok(false);
	}

	let mut file = File::open(path).await?;
	let mut count = 0;

	while let Ok(u) = file.read_u8().await {
		count += 1;
		if count >= 6000 {
			break;
		}
		if u == b'\x00' || u == b'\xff' {
			return Ok(false);
		}
	}

	Ok(true)
}
