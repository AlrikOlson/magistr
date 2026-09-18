//! Selected record content shared by surfaces; no layout or drawing decisions.
use crate::pixel::Icon;
use crate::widget_id::WidgetId;

pub struct Inspection {
    pub title: String,
    pub status: String,
    pub icon: Icon,
    pub metadata: String,
    pub actions: Vec<(&'static str, WidgetId)>,
    pub fields: Vec<(String, String)>,
    pub diff: Option<String>,
    pub viewer: Option<Vec<String>>,
    pub proposals: Vec<(usize, String)>,
}
