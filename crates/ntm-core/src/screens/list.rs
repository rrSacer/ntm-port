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

    /// NTM's selector over the stations' recipe tables.
    pub fn selector() -> Self {
        Self {
            source: "native:recipes".to_owned(),
            rect: selector::GRID,
            columns: selector::COLUMNS,
            rows: selector::ROWS,
            button_base: crate::native::RECIPE_BASE,
            clear_button: crate::native::RECIPE_CLEAR,
        }
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

/// NTM's `GUIScreenRecipeSelector`, in its texture's pixels: the window, the
/// buttons that sit over it (their hover sprites are at `uv`), the selection's
/// icon and the search field.
pub mod selector {
    pub const SPRITE: &str = "ntm:gui/processing/gui_recipe_selector";
    pub const SIZE: [f32; 2] = [176.0, 132.0];
    pub const GRID: [f32; 4] = [7.0, 17.0, 144.0, 90.0];
    pub const COLUMNS: usize = 8;
    pub const ROWS: usize = 5;
    /// A button: its rect, and where its hovered picture is in the sprite.
    pub struct Button {
        pub rect: [f32; 4],
        pub uv: [f32; 2],
    }
    pub const UP: Button = Button {
        rect: [152.0, 18.0, 16.0, 16.0],
        uv: [176.0, 0.0],
    };
    pub const DOWN: Button = Button {
        rect: [152.0, 36.0, 16.0, 16.0],
        uv: [176.0, 16.0],
    };
    pub const CLOSE: Button = Button {
        rect: [152.0, 90.0, 16.0, 16.0],
        uv: [176.0, 32.0],
    };
    pub const CLEAR_SEARCH: Button = Button {
        rect: [134.0, 108.0, 16.0, 16.0],
        uv: [176.0, 48.0],
    };
    /// Only a hover hint: Enter does it.
    pub const FOCUS_HINT: Button = Button {
        rect: [8.0, 108.0, 16.0, 16.0],
        uv: [176.0, 64.0],
    };
    /// The chosen recipe's icon is drawn at `SELECTED_ICON`; pressing its
    /// frame clears the choice.
    pub const SELECTED_FRAME: [f32; 4] = [151.0, 71.0, 18.0, 18.0];
    pub const SELECTED_ICON: [f32; 2] = [152.0, 72.0];
    /// A chosen recipe's frame in the grid, 18 square.
    pub const SELECTED_CELL_UV: [f32; 2] = [192.0, 0.0];
    /// The search field's text box, and the picture drawn while it has focus.
    pub const SEARCH_TEXT: [f32; 4] = [28.0, 111.0, 102.0, 12.0];
    pub const SEARCH_FOCUS: [f32; 4] = [26.0, 108.0, 106.0, 16.0];
    pub const SEARCH_FOCUS_UV: [f32; 2] = [0.0, 132.0];
    /// `setMaxStringLength(32)`.
    pub const SEARCH_MAX: usize = 32;

    /// A press on a rect: its left and bottom edges count, not its right or
    /// top, as the selector's whole-pixel tests do.
    pub fn hits(rect: [f32; 4], x: f32, y: f32) -> bool {
        let [left, top, w, h] = rect;
        x >= left && x < left + w && y > top && y <= top + h
    }
}

/// `GuiTextField` as the selector uses it: a line of at most
/// [`selector::SEARCH_MAX`] characters with a caret, and focus.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Search {
    text: String,
    /// In characters.
    caret: usize,
    pub focused: bool,
}

impl Search {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    fn chars(&self) -> usize {
        self.text.chars().count()
    }

    fn byte(&self, caret: usize) -> usize {
        self.text
            .char_indices()
            .nth(caret)
            .map_or(self.text.len(), |(i, _)| i)
    }

    /// Typed text at the caret, up to the limit; control characters are not
    /// typed. True when it changed the text.
    pub fn insert(&mut self, typed: &str) -> bool {
        let mut changed = false;
        for c in typed.chars().filter(|c| !c.is_control()) {
            if self.chars() >= selector::SEARCH_MAX {
                break;
            }
            let at = self.byte(self.caret);
            self.text.insert(at, c);
            self.caret += 1;
            changed = true;
        }
        changed
    }

    pub fn backspace(&mut self) -> bool {
        if self.caret == 0 {
            return false;
        }
        self.caret -= 1;
        let at = self.byte(self.caret);
        self.text.remove(at);
        true
    }

    pub fn delete(&mut self) -> bool {
        if self.caret >= self.chars() {
            return false;
        }
        let at = self.byte(self.caret);
        self.text.remove(at);
        true
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.caret = 0;
    }

    /// The part of the text that fits `width` pixels ending at the caret,
    /// as `(first character, text)`, given each character's width.
    pub fn visible(&self, width: f32, char_width: impl Fn(char) -> f32) -> (usize, String) {
        let chars: Vec<char> = self.text.chars().collect();
        let mut start = self.caret;
        let mut used = 0.0;
        while start > 0 && used + char_width(chars[start - 1]) <= width {
            start -= 1;
            used += char_width(chars[start]);
        }
        let mut end = self.caret;
        while end < chars.len() && used + char_width(chars[end]) <= width {
            used += char_width(chars[end]);
            end += 1;
        }
        (start, chars[start..end].iter().collect())
    }
}

/// What a selector does with an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Taken, and the view stays.
    Taken,
    /// Taken: the view closes (and sends [`State::commit`]).
    Close,
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
    pub search: Search,
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
            search: Search::default(),
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

    /// A press at `(x, y)` of the window (either button): the search field
    /// takes focus when it is on it, the arrows scroll a row, a recipe is
    /// chosen or not, and the frame of the chosen recipe clears it.
    pub fn press(&mut self, x: f32, y: f32) -> Outcome {
        self.search.focused = selector::hits(selector::SEARCH_TEXT, x, y);
        let on = |b: &selector::Button| selector::hits(b.rect, x, y);
        if on(&selector::UP) {
            self.navigate(Navigate::Up);
        } else if on(&selector::DOWN) {
            self.navigate(Navigate::Down);
        } else if on(&selector::CLEAR_SEARCH) {
            self.search.clear();
            self.filter("");
            self.search.focused = true;
        } else if let Some(index) = self.entry_at(x, y).map(|e| e.index) {
            self.choose(index);
        } else if selector::hits(selector::SELECTED_FRAME, x, y) {
            self.clear();
        } else if on(&selector::CLOSE) {
            return Outcome::Close;
        }
        Outcome::Taken
    }

    /// A key going down. `key` is `key.keyboard.enter`, `text` what it
    /// types, `inventory_key` the key bound to the inventory. As the
    /// selector's `keyTyped`: Enter toggles the search field's focus; a
    /// focused field takes its keys first (E is a letter then); then the
    /// arrows, Page and Home/End move the view; Escape or the inventory key
    /// close. Every key is taken: the selector is a window of its own.
    pub fn key(&mut self, key: &str, text: &str, inventory_key: &str) -> Outcome {
        if key == "key.keyboard.enter" {
            self.search.focused = !self.search.focused;
            return Outcome::Taken;
        }
        if self.search.focused && self.edit(key, text) {
            self.refilter();
            return Outcome::Taken;
        }
        match key {
            "key.keyboard.up" => self.navigate(Navigate::Up),
            "key.keyboard.down" => self.navigate(Navigate::Down),
            "key.keyboard.page.up" => self.navigate(Navigate::PageUp),
            "key.keyboard.page.down" => self.navigate(Navigate::PageDown),
            "key.keyboard.home" => self.navigate(Navigate::Home),
            "key.keyboard.end" => self.navigate(Navigate::End),
            _ => {}
        }
        if key == "key.keyboard.escape" || key == inventory_key {
            return Outcome::Close;
        }
        Outcome::Taken
    }

    /// Filters by what the search field holds.
    fn refilter(&mut self) {
        let text = self.search.text.clone();
        self.filter(&text);
    }

    /// What a focused field does with a key: true when it took it.
    fn edit(&mut self, key: &str, text: &str) -> bool {
        let search = &mut self.search;
        match key {
            "key.keyboard.backspace" => {
                search.backspace();
                true
            }
            "key.keyboard.delete" => {
                search.delete();
                true
            }
            "key.keyboard.left" => {
                search.caret = search.caret.saturating_sub(1);
                true
            }
            "key.keyboard.right" => {
                search.caret = (search.caret + 1).min(search.chars());
                true
            }
            "key.keyboard.home" => {
                search.caret = 0;
                true
            }
            "key.keyboard.end" => {
                search.caret = search.chars();
                true
            }
            _ => {
                !text.is_empty() && {
                    search.insert(text);
                    true
                }
            }
        }
    }

    /// Text typed without keys (a script): into a focused field; `\u{8}`
    /// is a Backspace. Taken either way.
    pub fn text(&mut self, typed: &str) -> Outcome {
        if self.search.focused {
            let changed = if typed == "\u{8}" {
                self.search.backspace()
            } else {
                self.search.insert(typed)
            };
            if changed {
                self.refilter();
            }
        }
        Outcome::Taken
    }

    /// The wheel: one row per event, up for positive `steps`.
    pub fn wheel(&mut self, steps: f32) -> Outcome {
        if steps > 0.0 {
            self.navigate(Navigate::Up);
        } else if steps < 0.0 {
            self.navigate(Navigate::Down);
        }
        Outcome::Taken
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

    fn selector_state(n: usize, selected: Option<u32>) -> State {
        State::new(Config::selector(), entries(n), selected)
    }

    #[test]
    fn the_selector_follows_ntm_geometry_and_pointer_edges() {
        let mut s = selector_state(100, None);
        // The first cell is x 7..25, y 17..35 including the bottom edge.
        assert_eq!(s.entry_at(7.0, 35.0).unwrap().index, 0);
        assert!(s.entry_at(7.0, 17.0).is_none());
        assert!(s.entry_at(25.0, 18.0).unwrap().index == 1);
        // Arrow buttons scroll one row; Up is a no-op at the top.
        assert_eq!(s.press(160.0, 20.0), Outcome::Taken);
        assert_eq!(s.offset, 0);
        assert_eq!(s.press(160.0, 38.0), Outcome::Taken);
        assert_eq!(s.offset, 1);
        assert_eq!(s.visible().next().unwrap().index, 8);
        // Close is the lower right button; a press outside everything is taken.
        assert_eq!(s.press(160.0, 100.0), Outcome::Close);
        assert_eq!(s.press(2.0, 2.0), Outcome::Taken);
        // Pressing the frame of the chosen recipe clears it.
        s.press(8.0, 20.0);
        let first = s.visible().next().unwrap().index;
        assert_eq!(first, 8);
        assert_eq!(s.selected, Some(first));
        assert_eq!(s.press(160.0, 80.0), Outcome::Taken);
        assert_eq!(s.selected, None);
    }

    #[test]
    fn the_search_field_takes_its_keys_before_the_view_does() {
        let e = "key.keyboard.e";
        let mut s = selector_state(100, None);
        // Unfocused, typing is ignored and E closes.
        assert_eq!(s.key("key.keyboard.r", "r", e), Outcome::Taken);
        assert_eq!(s.search.text(), "");
        // Enter focuses; E is a letter and Escape still closes.
        s.key("key.keyboard.enter", "", e);
        assert!(s.search.focused);
        s.key("key.keyboard.r", "R", e);
        s.key("key.keyboard.e", "e", e);
        assert_eq!(s.search.text(), "Re");
        assert_eq!(s.visible().count(), 40, "\"re\" is in every label");
        for c in "cipe 9".chars() {
            s.key("key.keyboard.x", &c.to_string(), e);
        }
        assert_eq!(s.visible().next().unwrap().index, 9);
        s.key("key.keyboard.backspace", "", e);
        s.key("key.keyboard.home", "", e);
        s.key("key.keyboard.delete", "", e);
        assert_eq!(s.search.text(), "ecipe ");
        // Page keys still scroll while focused; Home/End belong to the field.
        s.key("key.keyboard.page.down", "", e);
        assert_eq!(s.offset, 5);
        assert_eq!(s.key("key.keyboard.escape", "", e), Outcome::Close);
        s.key("key.keyboard.enter", "", e);
        assert!(!s.search.focused);
        assert_eq!(s.key("key.keyboard.e", "e", e), Outcome::Close);
        // The limit is 32 characters.
        let mut s = selector_state(10, None);
        s.search.focused = true;
        s.text(&"a".repeat(40));
        assert_eq!(s.search.text().len(), selector::SEARCH_MAX);
        s.text("\u{8}");
        assert_eq!(s.search.text().len(), selector::SEARCH_MAX - 1);
        // Typed text needs focus, scripts included; the clear button focuses.
        s.press(140.0, 120.0);
        assert_eq!(s.search.text(), "");
        assert!(s.search.focused);
        s.press(2.0, 2.0);
        assert!(!s.search.focused);
        s.text("zzz");
        assert_eq!(s.search.text(), "");
    }

    #[test]
    fn the_wheel_and_home_end_move_a_row_and_the_ends() {
        let mut s = selector_state(100, None);
        s.wheel(-1.0);
        s.wheel(-3.0);
        assert_eq!(s.offset, 2);
        s.wheel(1.0);
        assert_eq!(s.offset, 1);
        s.wheel(0.0);
        assert_eq!(s.offset, 1);
        s.key("key.keyboard.end", "", "key.keyboard.e");
        assert_eq!(s.offset, s.last_offset());
        s.key("key.keyboard.home", "", "key.keyboard.e");
        assert_eq!(s.offset, 0);
        // A tentative choice commits once, when it differs from the opening.
        s.press(8.0, 20.0);
        assert_eq!(s.commit(), Some(10000));
    }

    #[test]
    fn the_visible_search_text_ends_at_the_caret() {
        let mut f = Search::default();
        f.insert("abcdef");
        let (start, text) = f.visible(18.0, |_| 6.0);
        assert_eq!((start, text.as_str()), (3, "def"));
        f.caret = 2;
        let (start, text) = f.visible(18.0, |_| 6.0);
        assert_eq!((start, text.as_str()), (0, "abc"));
    }
}
