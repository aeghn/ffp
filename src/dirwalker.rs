use std::time::SystemTime;

use flume::Sender;
use futures_util::StreamExt;
use tracing::{error, warn};

use crate::{fileinfo::FileInfo, ui::finder::FinderIn};

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
	let mut items: Vec<FileInfo> = Vec::with_capacity(50000);
	if let Err(err) = tx.send_async(FinderIn::Clear).await {
		tracing::error!("unable to send clear msg, {}", err);
	}
	match filter.find_type {
		FindType::LS =>
			if let Ok(mut dir) = tokio::fs::read_dir(cwd).await {
				while let Ok(Some(en)) = dir.next_entry().await {
					let path = en.path();
					let info = FileInfo::new(path, cwd, None);
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
						items.push(FileInfo::new(de.path(), cwd, None));
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
