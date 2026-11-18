//! What a block shows beside the crosshair while a player looks at it (`blocks/ILookOverlay.java`), drawn by
//! `ModEventHandlerClient:205-221` on the crosshair pass when `ClientConfig.DODD_RBMK_DIAGNOSTIC` is on (its default;
//! the port has no client config: always on). Only the blocks whose overlay is ported are here.
//!
//! `ILookOverlay.printGeneric` (lines 12-48): the title at `(width / 2 + 8, height / 2 - 10)` in its color over its
//! background color one pixel down and right, then each line from `height / 2` down, 10 apart, white with a shadow
//! unless it starts with `&[color&]`.

/// A block's overlay: `printGeneric(event, title, titleCol, bgCol, text)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub title: String,
    pub title_color: u32,
    pub background: u32,
    pub lines: Vec<String>,
}

/// The overlay of the block with id `block`, if it has one in the port.
pub fn of_block(block: &str) -> Option<Overlay> {
    match block {
        // `MachineIGenerator.printHook` (lines 84-89).
        "ntm:machine_industrial_generator" => Some(Overlay {
            title: "Industrial Generator Memorial".into(),
            title_color: 0xff8000,
            background: 0x804000,
            lines: vec!["In memory of all that we have lost".into()],
        }),
        _ => None,
    }
}

/// `mc.objectMouseOver`'s reach (`PlayerControllerMP.getBlockReachDistance`): 5 in creative, 4.5 otherwise.
pub fn reach(creative: bool) -> f32 {
    if creative { 5.0 } else { 4.5 }
}

/// A line's color and text: `&[n&]` before it sets the color (`Integer.parseInt` of what is between), white else. A
/// color that does not parse is drawn as text (the Java draws the exception's name in red and stops).
pub fn line_color(line: &str) -> (u32, &str) {
    if let Some(rest) = line.strip_prefix("&[")
        && let Some(end) = rest.rfind("&]")
        && let Ok(color) = rest[..end].parse::<i32>()
    {
        return (color as u32, &rest[end + 2..]);
    }
    (0xFFFFFF, line)
}

/// Where `printGeneric` draws on a screen of `width` x `height` (scaled): the title's background, the title, and
/// each line, as (x, y).
pub fn layout(width: f32, height: f32, lines: usize) -> ([f32; 2], [f32; 2], Vec<[f32; 2]>) {
    let x = (width as i32 / 2 + 8) as f32;
    let z = (height as i32 / 2) as f32;
    ([x + 1.0, z - 9.0], [x, z - 10.0], (0..lines).map(|i| [x, z + 10.0 * i as f32]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_industrial_generator_is_a_memorial() {
        let o = of_block("ntm:machine_industrial_generator").unwrap();
        assert_eq!(o.title, "Industrial Generator Memorial");
        assert_eq!((o.title_color, o.background), (0xff8000, 0x804000));
        assert_eq!(o.lines, ["In memory of all that we have lost"]);
        assert_eq!(of_block("ntm:machine_lpw2"), None);
    }

    #[test]
    fn the_reach_is_the_controllers() {
        assert_eq!(reach(true), 5.0);
        assert_eq!(reach(false), 4.5);
    }

    #[test]
    fn a_line_may_name_its_color() {
        assert_eq!(line_color("plain"), (0xFFFFFF, "plain"));
        assert_eq!(line_color("&[16711680&]red"), (0xFF0000, "red"));
        assert_eq!(line_color("&[x&]bad"), (0xFFFFFF, "&[x&]bad"));
    }

    #[test]
    fn it_is_drawn_right_of_the_crosshair() {
        let (shadow, title, lines) = layout(427.0, 240.0, 2);
        assert_eq!(title, [221.0, 110.0]);
        assert_eq!(shadow, [222.0, 111.0]);
        assert_eq!(lines, [[221.0, 120.0], [221.0, 130.0]]);
    }
}
