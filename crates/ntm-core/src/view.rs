//! Machine screens as view-models: the server says what a screen holds, the
//! client turns it into `ui` widgets (docs/fundacao.md, "Telas").

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewModel {
    pub title: String,
    pub widgets: Vec<Widget>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "w")]
pub enum Widget {
    Text { text: String, color: u32 },
    /// A gauge: drawn as a text bar.
    Bar {
        label: String,
        value: u64,
        max: u64,
        unit: String,
    },
    /// A click is [`crate::proto::C2S::Action`] with this `action`.
    Button { action: u32, label: String },
    Item { item: String, name: String },
    /// Rows to pick one of (a `ui` list): picking row `r` is the action
    /// `action + r`.
    List {
        action: u32,
        rows: Vec<String>,
        selected: Option<u32>,
    },
}

/// The actions lists use: `LIST_ACTIONS + row`, below the text ids.
pub const LIST_ACTIONS: u32 = 5_000;
/// Rows a `ui` list may have.
pub const LIST_MAX: usize = 512;

pub const GREY: u32 = 0xA0A0A0;
pub const GREEN: u32 = 0x55FF55;
pub const RED: u32 = 0xFF5555;
pub const YELLOW: u32 = 0xFFFF55;

impl ViewModel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            widgets: Vec::new(),
        }
    }
    pub fn text(mut self, text: impl Into<String>, color: u32) -> Self {
        self.widgets.push(Widget::Text {
            text: text.into(),
            color,
        });
        self
    }
    pub fn bar(mut self, label: &str, value: u64, max: u64, unit: &str) -> Self {
        self.widgets.push(Widget::Bar {
            label: label.into(),
            value,
            max,
            unit: unit.into(),
        });
        self
    }
    pub fn button(mut self, action: u32, label: impl Into<String>) -> Self {
        self.widgets.push(Widget::Button {
            action,
            label: label.into(),
        });
        self
    }
    pub fn list(mut self, action: u32, rows: Vec<String>, selected: Option<u32>) -> Self {
        let mut rows = rows;
        rows.truncate(LIST_MAX);
        self.widgets.push(Widget::List { action, rows, selected });
        self
    }
    pub fn item(mut self, item: &str, name: impl Into<String>) -> Self {
        self.widgets.push(Widget::Item {
            item: item.into(),
            name: name.into(),
        });
        self
    }
}

/// `1.2M`, `340k`, `999`.
pub fn si(v: u64) -> String {
    match v {
        0..1_000 => v.to_string(),
        1_000..1_000_000 => trim(v as f64 / 1e3, "k"),
        1_000_000..1_000_000_000 => trim(v as f64 / 1e6, "M"),
        _ => trim(v as f64 / 1e9, "G"),
    }
}
fn trim(x: f64, unit: &str) -> String {
    let s = format!("{x:.1}");
    format!("{}{unit}", s.strip_suffix(".0").unwrap_or(&s))
}

/// `Energia [#####-----] 500k/1M HE`: fits a 200-pixel widget for gauges
/// up to `999.9M`.
pub fn bar_text(label: &str, value: u64, max: u64, unit: &str) -> String {
    const CELLS: u64 = 10;
    let filled = (value.min(max) * CELLS).checked_div(max).unwrap_or(0);
    let bar: String = (0..CELLS)
        .map(|i| if i < filled { '#' } else { '-' })
        .collect();
    format!("{label} [{bar}] {}/{} {unit}", si(value), si(max))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats() {
        assert_eq!(si(1_000_000), "1M");
        assert_eq!(si(450_000), "450k");
        assert_eq!(si(1_250), "1.2k");
        assert_eq!(bar_text("E", 500, 1000, "HE"), "E [#####-----] 500/1k HE");
        assert_eq!(bar_text("E", 5, 0, "HE"), "E [----------] 5/0 HE");
    }
}
