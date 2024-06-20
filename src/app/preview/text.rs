use std::{path::Path, sync::Arc};

use chin_tools::wrapper::anyhow::RResult;
use ratatui::{
	layout::{Constraint, Layout, Rect},
	text::{Line, Span, Text},
	widgets::{Paragraph, Wrap},
	Frame
};
use syntect::{
	easy::HighlightLines,
	highlighting::ThemeSet,
	parsing::{SyntaxReference, SyntaxSet},
	util::LinesWithEndings
};
use tokio::{
	fs::File,
	io::{AsyncBufReadExt, BufReader}
};

use super::{FileInfoHandleErr, ViewMsg, Viewer};
use crate::{dirwalker::read_first_n_chars, fileinfo::FileInfo, vendor::syntect_tui::into_span};

pub struct TextHighlighter {
	syntaxes: SyntaxSet
}

impl TextHighlighter {
	fn new() -> Self {
		Self {
			syntaxes: Default::default()
		}
	}

	// like https://github.com/sxyazi/yazi/blob/main/yazi-plugin/src/external/highlighter.rs
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

	async fn translate(
		&self,
		filepath: &Path,
		file_content: String
	) -> RResult<Vec<Vec<Span<'static>>>> {
		match self.translate_style(filepath, file_content.as_str()).await {
			Ok(vec) => Ok(vec),
			Err(_) => Ok(self.translate_plain(&file_content))
		}
	}

	fn translate_plain(&self, content: &str) -> Vec<Vec<Span<'static>>> {
		content
			.split("\n")
			.map(|line| {
				vec![Span {
					content: line.replace("\t", "  ").into(),
					style: Default::default()
				}]
			})
			.collect::<Vec<Vec<Span<'static>>>>()
	}

	async fn translate_style(
		&self,
		filepath: &Path,
		content: &str
	) -> RResult<Vec<Vec<Span<'static>>>> {
		let ps = SyntaxSet::load_defaults_newlines();
		let ts = ThemeSet::load_defaults();
		let syntax = self.detect_syntax(filepath).await?;
		let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.light"]);
		let mut lines: Vec<Vec<Span<'static>>> = vec![];

		for line in LinesWithEndings::from(content) {
			// LinesWithEndings enables use of newlines mode
			let line_spans: Vec<Span> = h
				.highlight_line(line, &ps)
				.unwrap()
				.into_iter()
				.filter_map(|(style, s)| into_span((style, s.replace("\t", "  ").as_str())).ok())
				.collect();
			lines.push(line_spans);
		}

		Ok(lines)
	}
}

pub struct TextViewer {
	highlighter: Arc<TextHighlighter>,
	wrap: bool
}

impl TextViewer {
	pub fn new() -> Self {
		Self {
			highlighter: Arc::new(TextHighlighter::new()),
			wrap: false
		}
	}
}

impl Viewer for TextViewer {
	async fn handle_fileinfo<F>(
		&self,
		fileinfo: FileInfo,
		cancel_signal: F
	) -> Result<ViewMsg, FileInfoHandleErr>
	where
		F: Fn() -> bool + Clone
	{
		let text = match read_first_n_chars(fileinfo.path(), 5000).await {
			Ok(content) => content,
			Err(err) => return Err(FileInfoHandleErr::TextReadErr(err.to_string()))
		};

		let text = self
			.highlighter
			.translate(fileinfo.path(), text)
			.await
			.unwrap()
			.into_iter()
			.map(|spans| Line::from(spans))
			.collect::<Text<'static>>();
		let paragraph = if self.wrap {
			Paragraph::new(text).wrap(Wrap::default())
		} else {
			Paragraph::new(text)
		};

		let attrs = super::regular_file_attrs(&fileinfo);

		if cancel_signal() {
			Err(FileInfoHandleErr::Cancelled)
		} else {
			Ok(ViewMsg {
				fileinfo,
				body: super::ViewType::Text(paragraph),
				attr: attrs
			})
		}
	}

	fn handle_event(&mut self, event: crossterm::event::Event) {}

	fn draw(&self, view_msg: &ViewMsg, cursor: u16, f: &mut Frame, rect: &Rect) {
		let attrs = view_msg.attr.as_ref();
		let attrs_height = attrs
			.as_ref()
			.map(|e| e.line_count(rect.width))
			.unwrap_or(0)
			.clamp(0, 5) as u16;

		let tb =
			Layout::vertical([Constraint::Fill(1), Constraint::Max(attrs_height)]).split(*rect);

		match &view_msg.body {
			super::ViewType::Text(text) => {
				let text = text.clone().scroll((cursor as u16, 0));

				f.render_widget(text, tb[0])
			}
			_ => {}
		}

		attrs.map(|e| f.render_widget(e.clone(), tb[1]));
	}
}
