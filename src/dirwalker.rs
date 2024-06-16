
use flume::Sender;
use futures_util::StreamExt;
use tracing::{error, warn};

use crate::{fileinfo::FileInfo, ui::finder::FinderIn};

#[derive(Clone, Default)]
pub enum FindType {
	#[default]
	LS,
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

pub fn rebuild_dirlist_start(sender: Sender<FinderIn>, cwd: &str, filter: DirFilter) {
	let cwd = cwd.to_string();
	tokio::spawn(async move {
		let cwd_ref = cwd.as_str();
		walk_dir(sender, cwd_ref, filter).await
	});
}

pub async fn walk_dir(tx: Sender<FinderIn>, cwd: &str, filter: DirFilter) {
	let mut items: Vec<FileInfo> = vec![];
	if let Err(err) = tx.send_async(FinderIn::Clear).await {
		tracing::error!("unable to send clear msg, {}", err);
	}
	match filter.find_type {
		FindType::LS =>
			if let Ok(mut dir) = tokio::fs::read_dir(cwd).await {
				while let Ok(Some(en)) = dir.next_entry().await {
					let path = en.path();
					match path.as_os_str().to_str() {
						Some(path_str) => {
							let info = FileInfo::new(
								path_str,
								diff_path(cwd, path_str),
								path.metadata().ok()
							);
							items.push(info);
							if items.len() > 200000 {
								tx.send_async(FinderIn::ContentsExtend(items))
									.await
									.map_err(|err| {
										error!("unable to send content extend msg: {}", err)
									})
									.ok();
								items = vec![];
							}
						}
						None => {}
					}
				}
			},
		FindType::FIND => {
			let mut wd = async_walkdir::WalkDir::new(cwd);
			while let Some(Ok(en)) = wd.next().await {
				let path = en.path();
				if let Some(path_str) = path.to_str() {
					items.push(FileInfo::new(
						path_str,
						diff_path(cwd, path_str),
						en.metadata().await.ok()
					));
					if items.len() > 200000 {
						tx.send_async(FinderIn::ContentsExtend(items))
							.await
							.map_err(|err| error!("unable to send content extend msg: {}", err))
							.ok();
						items = vec![];
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
