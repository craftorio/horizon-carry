//! Horses and their kin, and llamas.

use super::{Look, Mob, nth};
use crate::prelude::*;

pub(super) fn look(m: &Mob) -> Option<Look> {
    let (name, b) = (m.name, m.b);
    Some(match name {
        "horse" => {
            const COLORS: [&str; 7] = ["white", "creamy", "chestnut", "brown", "black", "gray", "darkbrown"];
            const MARKINGS: [&str; 5] = ["", "white", "whitefield", "whitedots", "blackdots"];
            // `Variant`: the colour in the low byte, the markings in the next one
            let packed = int(m.data, "Variant").unwrap_or(0);
            let color = m.v().unwrap_or_else(|| nth(&COLORS, Some(packed & 0xFF)));
            let mut look = m.one(format!("horse{b}"), format!("entity/horse/horse_{color}{b}"));
            let marks = nth(&MARKINGS, Some((packed >> 8) & 0xFF));
            if !marks.is_empty() {
                look.extend(m.one(format!("horse{b}"), format!("entity/horse/horse_markings_{marks}{b}")));
            }
            look
        }
        "donkey" | "mule" => m.one(format!("{name}{b}"), format!("entity/horse/{name}{b}")),
        "skeleton_horse" => m.one(format!("skeleton_horse{b}"), format!("entity/horse/horse_skeleton{b}")),
        "zombie_horse" => m.one(format!("zombie_horse{b}"), format!("entity/horse/horse_zombie{b}")),
        "llama" | "trader_llama" => {
            const COLORS: [&str; 4] = ["creamy", "white", "brown", "gray"];
            let color = m.v().unwrap_or_else(|| nth(&COLORS, int(m.data, "Variant")));
            m.one(format!("{name}{b}"), format!("entity/llama/llama_{color}{b}"))
        }
        _ => return None,
    })
}
