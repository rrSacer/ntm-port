//! `GuiTextField` of Minecraft 1.7.10 (RBMK T06): the text, its cursor and the other end of the selection, edited as
//! the Java edits them. The text is kept in UTF-16 units, as a Java `String` is, so lengths and positions are Java's.
//! Drawing, the cursor's blink and the scroll offset (`lineScrollOffset`, which only moves what is drawn) are the
//! screen's.

/// The keys a 1.7.10 field reads by their LWJGL code (`textboxKeyTyped`'s second argument).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// 14.
    Backspace,
    /// 199.
    Home,
    /// 203.
    Left,
    /// 205.
    Right,
    /// 207.
    End,
    /// 211.
    Delete,
    /// Any other key: only its character counts.
    Other,
}

/// What the clipboard gives and takes (`GuiScreen.getClipboardString`/`setClipboardString`).
pub trait Clipboard {
    fn get(&mut self) -> String;
    fn set(&mut self, text: &str);
}

/// No clipboard: pastes nothing, copies nowhere.
pub struct NoClipboard;

impl Clipboard for NoClipboard {
    fn get(&mut self) -> String {
        String::new()
    }
    fn set(&mut self, _: &str) {}
}

/// `ChatAllowedCharacters.isAllowedCharacter`: not `§`, not a control character, not DEL.
pub fn is_allowed(c: u16) -> bool {
    c != 167 && c >= 32 && c != 127
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextField {
    text: Vec<u16>,
    max: usize,
    cursor: usize,
    selection_end: usize,
    pub focused: bool,
    pub enabled: bool,
    pub can_lose_focus: bool,
    /// `enableBackgroundDrawing`: a click's x is taken 4 further in.
    pub background: bool,
}

impl Default for TextField {
    fn default() -> Self {
        // The constructor's defaults: 32 characters, enabled, losing focus on a click elsewhere, with a background.
        Self { text: Vec::new(), max: 32, cursor: 0, selection_end: 0, focused: false, enabled: true, can_lose_focus: true, background: true }
    }
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

impl TextField {
    pub fn new(max: usize) -> Self {
        let mut f = Self::default();
        f.set_max(max);
        f
    }

    pub fn text(&self) -> String {
        String::from_utf16_lossy(&self.text)
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// `getMaxStringLength`.
    pub fn max(&self) -> usize {
        self.max
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn selection_end(&self) -> usize {
        self.selection_end
    }

    /// `setMaxStringLength`: the text is cut to it (the cursor is not moved).
    pub fn set_max(&mut self, max: usize) {
        self.max = max;
        self.text.truncate(max);
    }

    /// `setText`: cut to the maximum, the cursor at the end.
    pub fn set_text(&mut self, text: &str) {
        let mut t = units(text);
        t.truncate(self.max);
        self.text = t;
        self.set_cursor_end();
    }

    /// `getSelectedText`.
    pub fn selected(&self) -> String {
        let (i, j) = (self.cursor.min(self.selection_end), self.cursor.max(self.selection_end));
        String::from_utf16_lossy(&self.text[i..j])
    }

    /// `writeText`: the selection replaced by `s` without its disallowed characters, cut to what still fits.
    pub fn write(&mut self, s: &str) {
        let s2: Vec<u16> = units(s).into_iter().filter(|c| is_allowed(*c)).collect();
        let i = self.cursor.min(self.selection_end);
        let j = self.cursor.max(self.selection_end);
        // `maxStringLength - text.length() - (i - selectionEnd)`, in Java's ints.
        let k = self.max as i64 - self.text.len() as i64 - (i as i64 - self.selection_end as i64);
        let mut out: Vec<u16> = Vec::new();
        if !self.text.is_empty() {
            out.extend_from_slice(&self.text[..i]);
        }
        let l = if k < s2.len() as i64 {
            // `substring(0, k)`: a negative k throws in Java; it cannot be negative here, since the text never passes
            // the maximum and the selection only gives room back.
            let k = k.max(0) as usize;
            out.extend_from_slice(&s2[..k]);
            k
        } else {
            out.extend_from_slice(&s2);
            s2.len()
        };
        if !self.text.is_empty() && j < self.text.len() {
            out.extend_from_slice(&self.text[j..]);
        }
        self.text = out;
        self.move_cursor_by(i as i64 - self.selection_end as i64 + l as i64);
    }

    /// `deleteWords`.
    pub fn delete_words(&mut self, n: i64) {
        if !self.text.is_empty() {
            if self.selection_end != self.cursor {
                self.write("");
            } else {
                let to = self.nth_word_from_cursor(n) as i64;
                self.delete_from_cursor(to - self.cursor as i64);
            }
        }
    }

    /// `deleteFromCursor`.
    pub fn delete_from_cursor(&mut self, n: i64) {
        if self.text.is_empty() {
            return;
        }
        if self.selection_end != self.cursor {
            self.write("");
            return;
        }
        let back = n < 0;
        let c = self.cursor as i64;
        let (j, k) = if back { (c + n, c) } else { (c, c + n) };
        let mut s = Vec::new();
        if j >= 0 {
            s.extend_from_slice(&self.text[..j as usize]);
        }
        if k < self.text.len() as i64 {
            s.extend_from_slice(&self.text[k as usize..]);
        }
        self.text = s;
        if back {
            self.move_cursor_by(n);
        }
    }

    /// `getNthWordFromCursor`.
    pub fn nth_word_from_cursor(&self, n: i64) -> usize {
        self.nth_word_from_pos(n, self.cursor)
    }

    /// `getNthWordFromPos`: the Java ignores `pos` and starts from the cursor.
    pub fn nth_word_from_pos(&self, n: i64, _pos: usize) -> usize {
        self.word_from(n, self.cursor, true)
    }

    /// `func_146197_a`: `n` words on from `from`, past the spaces after (or before) each when `skip`.
    fn word_from(&self, n: i64, from: usize, skip: bool) -> usize {
        let mut k = from;
        let back = n < 0;
        for _ in 0..n.unsigned_abs() {
            if back {
                while skip && k > 0 && self.text[k - 1] == 32 {
                    k -= 1;
                }
                while k > 0 && self.text[k - 1] != 32 {
                    k -= 1;
                }
            } else {
                let len = self.text.len();
                match self.text[k.min(len)..].iter().position(|c| *c == 32) {
                    None => k = len,
                    Some(p) => {
                        k += p;
                        while skip && k < len && self.text[k] == 32 {
                            k += 1;
                        }
                    }
                }
            }
        }
        k
    }

    /// `moveCursorBy`: from the selection's other end, as the Java does.
    pub fn move_cursor_by(&mut self, n: i64) {
        self.set_cursor(self.selection_end as i64 + n);
    }

    /// `setCursorPosition`: clamped to the text, the selection collapsed onto it.
    pub fn set_cursor(&mut self, at: i64) {
        self.cursor = at.clamp(0, self.text.len() as i64) as usize;
        self.set_selection(self.cursor as i64);
    }

    pub fn set_cursor_end(&mut self) {
        self.set_cursor(self.text.len() as i64);
    }

    /// `setSelectionPos`: clamped to the text.
    pub fn set_selection(&mut self, at: i64) {
        self.selection_end = at.clamp(0, self.text.len() as i64) as usize;
    }

    /// `setFocused`.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// `textboxKeyTyped(c, key)` with the modifiers held: whether the field took the key.
    pub fn key_typed(&mut self, c: u16, key: Key, ctrl: bool, shift: bool, clipboard: &mut dyn Clipboard) -> bool {
        if !self.focused {
            return false;
        }
        match c {
            1 => {
                self.set_cursor_end();
                self.set_selection(0);
                return true;
            }
            3 => {
                clipboard.set(&self.selected());
                return true;
            }
            22 => {
                if self.enabled {
                    let s = clipboard.get();
                    self.write(&s);
                }
                return true;
            }
            24 => {
                clipboard.set(&self.selected());
                if self.enabled {
                    self.write("");
                }
                return true;
            }
            _ => {}
        }
        match key {
            Key::Backspace => {
                if ctrl {
                    if self.enabled {
                        self.delete_words(-1);
                    }
                } else if self.enabled {
                    self.delete_from_cursor(-1);
                }
                true
            }
            Key::Home => {
                if shift {
                    self.set_selection(0);
                } else {
                    self.set_cursor(0);
                }
                true
            }
            Key::Left => {
                if shift {
                    if ctrl {
                        let to = self.nth_word_from_pos(-1, self.selection_end);
                        self.set_selection(to as i64);
                    } else {
                        self.set_selection(self.selection_end as i64 - 1);
                    }
                } else if ctrl {
                    let to = self.nth_word_from_cursor(-1);
                    self.set_cursor(to as i64);
                } else {
                    self.move_cursor_by(-1);
                }
                true
            }
            Key::Right => {
                if shift {
                    if ctrl {
                        let to = self.nth_word_from_pos(1, self.selection_end);
                        self.set_selection(to as i64);
                    } else {
                        self.set_selection(self.selection_end as i64 + 1);
                    }
                } else if ctrl {
                    let to = self.nth_word_from_cursor(1);
                    self.set_cursor(to as i64);
                } else {
                    self.move_cursor_by(1);
                }
                true
            }
            Key::End => {
                if shift {
                    self.set_selection(self.text.len() as i64);
                } else {
                    self.set_cursor_end();
                }
                true
            }
            Key::Delete => {
                if ctrl {
                    if self.enabled {
                        self.delete_words(1);
                    }
                } else if self.enabled {
                    self.delete_from_cursor(1);
                }
                true
            }
            Key::Other => {
                if is_allowed(c) {
                    if self.enabled {
                        self.write(&String::from_utf16_lossy(&[c]));
                    }
                    true
                } else {
                    false
                }
            }
        }
    }

    /// `mouseClicked(x, y, button)` for a field at `rect` (x, y, width, height): focus follows the click (when the field
    /// can lose it), and a left click in a focused field puts the cursor where `fit` says (`trimStringToWidth`: how
    /// many of the given units fit in a width).
    pub fn mouse_clicked(&mut self, x: i32, y: i32, button: i32, rect: [i32; 4], fit: impl Fn(&[u16], i32) -> usize) {
        let inside = x >= rect[0] && x < rect[0] + rect[2] && y >= rect[1] && y < rect[1] + rect[3];
        if self.can_lose_focus {
            self.set_focused(inside);
        }
        if self.focused && button == 0 {
            let mut l = x - rect[0];
            if self.background {
                l -= 4;
            }
            // `getWidth`: the width less 8 with a background.
            let width = if self.background { rect[2] - 8 } else { rect[2] };
            // The scroll offset only moves what is drawn; a field as short as the RBMK's never scrolls.
            let shown = fit(&self.text, width);
            let at = fit(&self.text[..shown], l);
            self.set_cursor(at as i64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(f: &mut TextField, s: &str) {
        for c in s.encode_utf16() {
            f.key_typed(c, Key::Other, false, false, &mut NoClipboard);
        }
    }

    fn key(f: &mut TextField, k: Key, ctrl: bool, shift: bool) {
        f.key_typed(0, k, ctrl, shift, &mut NoClipboard);
    }

    #[test]
    fn typing_stops_at_the_maximum_and_drops_what_chat_refuses() {
        let mut f = TextField::new(4);
        f.focused = true;
        typed(&mut f, "12§\u{7f}\u{1f}345");
        assert_eq!((f.text(), f.cursor()), ("1234".into(), 4));
        // Unfocused, nothing is taken.
        f.focused = false;
        assert!(!f.key_typed(u16::from(b'9'), Key::Other, false, false, &mut NoClipboard));
        assert_eq!(f.text(), "1234");
    }

    #[test]
    fn set_text_cuts_to_the_maximum_and_a_smaller_maximum_cuts_the_text() {
        let mut f = TextField::new(3);
        f.set_text("12345");
        assert_eq!((f.text(), f.cursor()), ("123".into(), 3));
        f.set_max(2);
        assert_eq!(f.text(), "12");
        // The cursor is not moved by `setMaxStringLength` (Java), and the next edit clamps it.
        assert_eq!(f.cursor(), 3);
    }

    #[test]
    fn backspace_delete_and_the_arrows_edit_at_the_cursor() {
        let mut f = TextField::new(10);
        f.focused = true;
        typed(&mut f, "abcd");
        key(&mut f, Key::Left, false, false);
        key(&mut f, Key::Backspace, false, false);
        assert_eq!((f.text(), f.cursor()), ("abd".into(), 2));
        key(&mut f, Key::Delete, false, false);
        assert_eq!((f.text(), f.cursor()), ("ab".into(), 2));
        key(&mut f, Key::Home, false, false);
        typed(&mut f, "x");
        assert_eq!((f.text(), f.cursor()), ("xab".into(), 1));
        key(&mut f, Key::End, false, false);
        assert_eq!(f.cursor(), 3);
    }

    #[test]
    fn a_selection_is_replaced_and_select_all_with_ctrl_a() {
        let mut f = TextField::new(4);
        f.focused = true;
        typed(&mut f, "1234");
        f.key_typed(1, Key::Other, true, false, &mut NoClipboard);
        assert_eq!((f.cursor(), f.selection_end()), (4, 0));
        assert_eq!(f.selected(), "1234");
        // Java's room is `max - length - (start - selectionEnd)`: with the cursor at the end and the selection's other
        // end at 0, a full field has none, and the typed character goes with the selection (1.7.10's quirk).
        typed(&mut f, "7");
        assert_eq!((f.text(), f.cursor()), (String::new(), 0));
        // Selected the other way (Home, then Shift+End), the room is the selection's.
        typed(&mut f, "1234");
        key(&mut f, Key::Home, false, false);
        key(&mut f, Key::End, false, true);
        typed(&mut f, "55");
        assert_eq!(f.text(), "55");
    }

    #[test]
    fn words_are_found_and_deleted_as_the_java_does() {
        let mut f = TextField::new(32);
        f.focused = true;
        typed(&mut f, "ab cd  ef");
        key(&mut f, Key::Backspace, true, false);
        assert_eq!(f.text(), "ab cd  ");
        key(&mut f, Key::Left, true, false);
        assert_eq!(f.cursor(), 3);
        key(&mut f, Key::Right, true, false);
        assert_eq!(f.cursor(), 7);
        key(&mut f, Key::Home, false, false);
        key(&mut f, Key::Delete, true, false);
        assert_eq!(f.text(), "cd  ");
    }

    #[test]
    fn the_clipboard_copies_cuts_and_pastes_through_the_filter() {
        struct Board(String);
        impl Clipboard for Board {
            fn get(&mut self) -> String {
                self.0.clone()
            }
            fn set(&mut self, t: &str) {
                self.0 = t.to_string();
            }
        }
        let mut b = Board("9§99999".into());
        let mut f = TextField::new(4);
        f.focused = true;
        f.key_typed(22, Key::Other, true, false, &mut b);
        assert_eq!(f.text(), "9999");
        f.key_typed(1, Key::Other, true, false, &mut b);
        f.key_typed(24, Key::Other, true, false, &mut b);
        assert_eq!((f.text(), b.0.as_str()), (String::new(), "9999"));
    }

    #[test]
    fn a_click_focuses_and_places_the_cursor() {
        let mut f = TextField::new(4);
        f.background = false;
        f.set_text("1234");
        // Four pixels a unit.
        let fit = |s: &[u16], w: i32| s.len().min((w.max(0) / 4) as usize);
        f.mouse_clicked(35, 30, 0, [30, 27, 26, 6], fit);
        assert!(f.focused);
        assert_eq!(f.cursor(), 1);
        f.mouse_clicked(10, 10, 0, [30, 27, 26, 6], fit);
        assert!(!f.focused);
    }

    #[test]
    fn lengths_are_java_utf16_units() {
        let mut f = TextField::new(4);
        f.focused = true;
        f.write("𝄞𝄞𝄞");
        assert_eq!(f.len(), 4, "two units each, the third cut");
    }
}
