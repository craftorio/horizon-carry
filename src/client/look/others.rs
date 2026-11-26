//! Mobs whose textures are not named after them, or not in a folder of their name.

use super::{Look, Mob};

pub(super) fn look(m: &Mob) -> Option<Look> {
    let b = m.b;
    Some(match m.name {
        "polar_bear" => m.one(format!("polar_bear{b}"), format!("entity/bear/polarbear{b}")),
        "happy_ghast" => m.one(format!("happy_ghast{b}"), format!("entity/ghast/happy_ghast{b}")),
        "sniffer" if m.baby => m.one("sniffer_baby", "entity/sniffer/snifflet"),
        _ => return None,
    })
}
