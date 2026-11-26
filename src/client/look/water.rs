//! Water mobs. Fish share `entity/fish/`, and their models go by size.

use super::{Look, Mob, dye, nth};
use crate::prelude::*;

pub(super) fn look(m: &Mob) -> Option<Look> {
    let b = m.b;
    Some(match m.name {
        "cod" => m.one("cod", "entity/fish/cod"),
        "salmon" => {
            let layer = match read_string(m.data, "type").as_deref() {
                Some("small") => "salmon_small",
                Some("large") => "salmon_large",
                _ => "salmon",
            };
            m.one(layer, "entity/fish/salmon")
        }
        "pufferfish" => {
            let layer = match int(m.data, "PuffState") {
                Some(2) => "pufferfish_big",
                Some(1) => "pufferfish_medium",
                _ => "pufferfish_small",
            };
            m.one(layer, "entity/fish/pufferfish")
        }
        "tropical_fish" => {
            // `Variant`: the shape (0 small, 1 large), its pattern, the body's and the pattern's dye colours
            let packed = int(m.data, "Variant").unwrap_or(0) as u32;
            let (size, letter) = if packed & 0xFF == 1 {
                ("large", "b")
            } else {
                ("small", "a")
            };
            let pattern = ((packed >> 8) & 0xFF) % 6 + 1;
            vec![
                (
                    format!("tropical_fish_{size}"),
                    format!("entity/fish/tropical_{letter}"),
                    dye(((packed >> 16) & 0xFF).into()),
                ),
                (
                    format!("tropical_fish_{size}#pattern"),
                    format!("entity/fish/tropical_{letter}_pattern_{pattern}"),
                    dye((packed >> 24).into()),
                ),
            ]
        }
        "glow_squid" => m.one(format!("glow_squid{b}"), format!("entity/squid/glow_squid{b}")),
        "axolotl" => {
            const NAMES: [&str; 5] = ["lucy", "wild", "gold", "cyan", "blue"];
            let variant = m.v().unwrap_or_else(|| nth(&NAMES, int(m.data, "Variant")));
            m.one(format!("axolotl{b}"), format!("entity/axolotl/axolotl_{variant}{b}"))
        }
        "frog" => m.one("frog", format!("entity/frog/frog_{}", m.v().unwrap_or("temperate"))),
        _ => return None,
    })
}
