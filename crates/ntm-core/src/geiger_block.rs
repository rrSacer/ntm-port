//! The Geiger counter block (`GeigerCounter`, `TileEntityGeiger`), Onda 1, slice 4b. Its block entity keeps nothing
//! (`TileEntityGeiger` has no NBT): its counters start at 0 each time it is made, placed or loaded.
//!
//! - `updateEntity:22-52`, every tick: `timer` counts to 10, then reads the world's radiation where the block is
//!   (`check`, `getRadiation`) into `ticker` and notifies the neighbours (the comparators read it again); every fifth
//!   tick a click from `ticker`'s list (the hand counter's, [`crate::instruments::geiger_pool`]) or, at 0, one in 50
//!   of `geiger1`.
//! - `getComparatorInputOverride:153-163`: `min(ceil(check() / 5F), 15)`, read when asked.
//! - `onBlockActivated:140-151`: not sneaking, the `techBoop` and the hand counter's report (`printGeigerData`).

/// `TileEntityGeiger`'s `timer` and `ticker`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeigerBlock {
    pub timer: u32,
    pub ticker: f32,
}

/// What a tick did: the neighbours notified, a click (`hbm:item.geiger<n>`) played at the block.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    pub notify: bool,
    pub click: Option<u8>,
}

impl GeigerBlock {
    /// `updateEntity`: `check` is `getRadiation` at the block, `next_int(n)` `worldObj.rand.nextInt(n)`.
    pub fn tick(&mut self, check: impl FnOnce() -> f32, mut next_int: impl FnMut(u32) -> u32) -> Step {
        let mut step = Step::default();
        self.timer += 1;
        if self.timer == 10 {
            self.timer = 0;
            self.ticker = check();
            step.notify = true;
        }
        if self.timer.is_multiple_of(5) {
            if self.ticker > 0.0 {
                let list = crate::instruments::geiger_pool(self.ticker);
                let r = list[next_int(list.len() as u32) as usize];
                if r > 0 {
                    step.click = Some(r);
                }
            } else if next_int(50) == 0 {
                // "hbm:item.geiger" + (1 + rand.nextInt(1)): always the first, the roll still taken.
                step.click = Some(1 + next_int(1) as u8);
            }
        }
        step
    }
}

/// `getComparatorInputOverride`: 0 at exactly 0 RAD/s, one more each 5 RAD/s, 15 from 75.
pub fn comparator(rad: f32) -> i32 {
    ((f64::from(rad / 5.0)).ceil() as i32).min(15)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_reads_every_10_ticks_and_clicks_every_5() {
        let mut g = GeigerBlock::default();
        let mut reads = 0;
        let mut notified = Vec::new();
        let mut clicked = Vec::new();
        for t in 1..=30 {
            let step = g.tick(
                || {
                    reads += 1;
                    12.0
                },
                |n| n - 1,
            );
            if step.notify {
                notified.push(t);
            }
            if step.click.is_some() {
                clicked.push(t);
            }
        }
        assert_eq!(reads, 3);
        assert_eq!(notified, [10, 20, 30]);
        // Before the first reading the ticker is 0: the idle roll, here never 0. From tick 10, 12 RAD/s: [1, 2, 3]
        // and the last pick, 3.
        assert_eq!(clicked, [10, 15, 20, 25, 30]);
        assert_eq!(g.ticker, 12.0);
    }

    #[test]
    fn at_zero_one_in_50_is_the_first_click_and_the_second_roll_is_taken() {
        let mut g = GeigerBlock::default();
        let mut rolls = Vec::new();
        for _ in 0..5 {
            g.tick(|| 0.0, |n| {
                rolls.push(n);
                0
            });
        }
        assert_eq!(rolls, [50, 1], "nextInt(50) == 0, then nextInt(1)");
        let mut g = GeigerBlock::default();
        let silent: Vec<_> = (0..5).map(|_| g.tick(|| 0.0, |_| 1).click).collect();
        assert_eq!(silent, [None; 5]);
        let mut g = GeigerBlock::default();
        let clicks: Vec<_> = (0..5).map(|_| g.tick(|| 0.0, |_| 0).click).collect();
        assert_eq!(clicks, [None, None, None, None, Some(1)]);
    }

    #[test]
    fn the_pick_is_from_the_tickers_list_and_entry_0_is_silent() {
        let mut g = GeigerBlock { timer: 9, ticker: 0.0 };
        // 0.5 RAD/s: [0, 0, 1]; the third entry clicks, the first is silence.
        assert_eq!(g.tick(|| 0.5, |n| n - 1).click, Some(1));
        let mut g = GeigerBlock { timer: 9, ticker: 0.0 };
        assert_eq!(g.tick(|| 0.5, |_| 0).click, None);
        let mut g = GeigerBlock { timer: 9, ticker: 0.0 };
        assert_eq!(g.tick(|| 40.0, |_| 0).click, Some(6));
    }

    #[test]
    fn the_comparator_steps_by_5_rad_up_to_15() {
        assert_eq!(comparator(0.0), 0);
        assert_eq!(comparator(0.001), 1);
        assert_eq!(comparator(5.0), 1);
        assert_eq!(comparator(5.01), 2);
        assert_eq!(comparator(74.9), 15);
        assert_eq!(comparator(100_000.0), 15);
    }
}
