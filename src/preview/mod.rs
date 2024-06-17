use ratatui::widgets::Widget;

pub mod text;

pub struct ViewReq {
	view_type: ViewType,
	path: String
}

pub enum ViewType {
	Text
}

pub trait Viewer {
	fn view(req: &ViewReq) -> impl Widget;
}
