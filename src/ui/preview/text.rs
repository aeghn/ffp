use std::path::Path;

use chin_tools::wrapper::anyhow::RResult;
use syntect::parsing::{SyntaxReference, SyntaxSet};
use tokio::{
	fs::File,
	io::{AsyncBufReadExt, BufReader}
};

pub struct TextHighlighter {
	syntaxes: SyntaxSet
}

impl TextHighlighter {
	// Copy from https://github.com/sxyazi/yazi/blob/main/yazi-plugin/src/external/highlighter.rs
	async fn detect_syntax(&self, filepath: &Path) -> RResult<&SyntaxReference> {
		if let Some(filename) = filepath
			.file_name()
			.map(|e| e.to_string_lossy().to_string())
		{
			if let Some(s) = self.syntaxes.find_syntax_by_extension(filename.as_str()) {
				return Ok(s);
			}
		}

		if let Some(ext) = filepath
			.extension()
			.map(|e| e.to_string_lossy().to_string())
		{
			if let Some(s) = self.syntaxes.find_syntax_by_extension(ext.as_str()) {
				return Ok(s);
			}
		}

		let mut line = String::new();
		let mut reader = BufReader::new(File::open(&filepath).await?);
		reader.read_line(&mut line).await?;
		self.syntaxes
			.find_syntax_by_first_line(&line)
			.ok_or_else(|| anyhow::anyhow!("No syntax found"))
	}

/* 	async fn view(&self, req: &super::ViewReq) -> RResult<impl ratatui::prelude::Widget> {
		let style = self.detect_syntax(req.path.as_path()).await;

		if let Ok(style) = style {}
	} */
}
