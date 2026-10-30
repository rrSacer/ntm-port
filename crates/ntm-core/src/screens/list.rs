//! A selectable icon grid for a `horizon_screen`: filtering and view position
//! are local; only a changed selection becomes a menu button on close.
//! Sources supply stable entry ids independently of filtering or scrolling.
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub index: u32,
    pub label: String,
    pub icon: String,
    pub inputs: Vec<(String, u32)>,
    pub outputs: Vec<(String, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub source: String,
    pub rect: [f32; 4],
    pub columns: usize,
    pub rows: usize,
    pub button_base: u32,
    pub clear_button: u32,
}

impl Config {
    pub fn parse(doc: &Value) -> Result<Self, String> {
        let count = |key: &str| -> Result<usize, String> {
            let n = doc[key]
                .as_u64()
                .ok_or_else(|| format!("list without {key}"))?;
            if !(1..=64).contains(&n) {
                return Err(format!("list {key} outside 1..=64"));
            }
            Ok(n as usize)
        };
        let rect = super::floats::<4>(&doc["rect"]).ok_or("list without rect")?;
        if !rect.iter().all(|v| v.is_finite()) || rect[2] <= 0.0 || rect[3] <= 0.0 {
            return Err("list rect must have finite coordinates and positive size".into());
        }
        let button = |key: &str| -> Result<u32, String> {
            doc[key]
                .as_u64()
                .and_then(|n| i32::try_from(n).ok())
                .map(|n| n as u32)
                .ok_or_else(|| format!("list {key} outside menu button range"))
        };
        Ok(Self {
            source: doc["source"]
                .as_str()
                .ok_or("list without source")?
                .to_owned(),
            rect,
            columns: count("columns")?,
            rows: count("rows")?,
            button_base: button("button_base")?,
            clear_button: button("clear_button")?,
        })
    }

    pub fn cell(&self, visible: usize) -> [f32; 4] {
        let [x, y, w, h] = self.rect;
        let (cw, ch) = (w / self.columns as f32, h / self.rows as f32);
        [
            x + (visible % self.columns) as f32 * cw,
            y + (visible / self.columns) as f32 * ch,
            cw,
            ch,
        ]
    }

    /// Each cell includes its left and bottom edges, but not its right or
    /// top, matching the selector's whole-pixel hit regions.
    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        let [left, top, w, h] = self.rect;
        if !(x >= left && x < left + w && y > top && y <= top + h) {
            return None;
        }
        let c = ((x - left) / (w / self.columns as f32)).floor() as usize;
        let r = ((y - top) / (h / self.rows as f32)).ceil() as usize - 1;
        Some(r * self.columns + c)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Navigate {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

pub struct State {
    pub config: Config,
    pub entries: Vec<Entry>,
    filtered: Vec<usize>,
    /// Rows skipped, not disjoint pages: adjacent views overlap.
    pub offset: usize,
    pub selected: Option<u32>,
    initial: Option<u32>,
    query: String,
}

impl State {
    pub fn new(config: Config, entries: Vec<Entry>, selected: Option<u32>) -> Self {
        let filtered = (0..entries.len()).collect();
        Self {
            config,
            entries,
            filtered,
            offset: 0,
            selected,
            initial: selected,
            query: String::new(),
        }
    }

    pub fn filter(&mut self, text: &str) {
        if self.query == text {
            return;
        }
        self.query = text.to_owned();
        let query = text.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.label.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        self.offset = 0;
    }

    pub fn last_offset(&self) -> usize {
        self.filtered
            .len()
            .saturating_sub(self.config.columns * self.config.rows)
            .div_ceil(self.config.columns)
    }

    pub fn navigate(&mut self, key: Navigate) {
        self.offset = match key {
            Navigate::Up => self.offset.saturating_sub(1),
            Navigate::Down => self.offset.saturating_add(1),
            Navigate::PageUp => self.offset.saturating_sub(self.config.rows),
            Navigate::PageDown => self.offset.saturating_add(self.config.rows),
            Navigate::Home => 0,
            Navigate::End => self.last_offset(),
        }
        .min(self.last_offset());
    }

    pub fn visible(&self) -> impl Iterator<Item = &Entry> {
        self.filtered
            .iter()
            .skip(self.offset * self.config.columns)
            .take(self.config.columns * self.config.rows)
            .map(|&i| &self.entries[i])
    }

    pub fn entry_at(&self, x: f32, y: f32) -> Option<&Entry> {
        let cell = self.config.hit(x, y)?;
        let &entry = self
            .filtered
            .get(self.offset * self.config.columns + cell)?;
        self.entries.get(entry)
    }

    pub fn choose(&mut self, index: u32) {
        if self.entries.iter().any(|e| e.index == index) {
            self.selected = (self.selected != Some(index)).then_some(index);
        }
    }

    pub fn clear(&mut self) {
        self.selected = None;
    }

    /// None is also returned for an invalid/overflowing protocol id, so a
    /// broken source cannot send a different selection by integer wrapping.
    pub fn commit(&self) -> Option<u32> {
        if self.selected == self.initial {
            return None;
        }
        match self.selected {
            None => Some(self.config.clear_button),
            Some(i) => self
                .config
                .button_base
                .checked_add(i)
                .filter(|&n| n <= i32::MAX as u32),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::parse(&serde_json::json!({
            "source":"native:recipes", "rect":[7,17,144,90],
            "columns":8, "rows":5, "button_base":10000, "clear_button":9999
        }))
        .unwrap()
    }

    fn entries(n: usize) -> Vec<Entry> {
        (0..n)
            .map(|i| Entry {
                index: i as u32,
                label: format!("Recipe {i}"),
                icon: "example:part".into(),
                inputs: Vec::new(),
                outputs: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn scrolling_is_one_row_and_last_partial_view_has_no_phantom_entry() {
        let mut s = State::new(config(), entries(49), None);
        assert_eq!(s.visible().count(), 40);
        s.navigate(Navigate::Down);
        assert_eq!(s.visible().next().unwrap().index, 8);
        s.navigate(Navigate::End);
        assert_eq!(s.offset, 2);
        assert_eq!(s.visible().count(), 33);
        assert!(s.entry_at(26.0, 90.0).is_none());
        s.navigate(Navigate::Down);
        assert_eq!(s.offset, 2);
        s.navigate(Navigate::PageUp);
        assert_eq!(s.offset, 0);
        assert_eq!(s.config.hit(7.0, 17.0), None);
        assert_eq!(s.config.hit(7.0, 35.0), Some(0));
        assert_eq!(s.config.hit(151.0, 18.0), None);
    }

    #[test]
    fn filtering_preserves_stable_ids_and_selection_is_sent_only_when_changed() {
        let mut s = State::new(config(), entries(100), Some(9));
        s.navigate(Navigate::End);
        s.filter("RECIPE 9");
        assert_eq!(s.offset, 0);
        assert_eq!(s.visible().next().unwrap().index, 9);
        assert_eq!(s.commit(), None);
        let chosen = s.entry_at(26.0, 18.0).unwrap().index;
        assert_eq!(chosen, 90); // Filtered position 1 is not recipe id 1.
        s.choose(chosen);
        assert_eq!(s.commit(), Some(10090));
        s.choose(chosen);
        assert_eq!(s.commit(), Some(9999));
        s.choose(9);
        assert_eq!(s.commit(), None);
        s.filter("no match");
        assert_eq!(s.visible().count(), 0);
        assert_eq!(s.selected, Some(9));
        assert_eq!(s.last_offset(), 0);
    }

    #[test]
    fn invalid_geometry_and_overflow_cannot_emit_a_wrapped_button() {
        let mut doc = serde_json::json!({"source":"x", "rect":[0,0,0,90],
            "columns":8,"rows":5,"button_base":10000,"clear_button":9999});
        assert!(Config::parse(&doc).is_err());
        doc["rect"] = serde_json::json!([0, 0, 144, 90]);
        doc["columns"] = serde_json::json!(0);
        assert!(Config::parse(&doc).is_err());
        let mut s = State::new(config(), entries(1), None);
        s.entries[0].index = u32::MAX;
        s.choose(u32::MAX);
        assert_eq!(s.commit(), None);
    }
}
