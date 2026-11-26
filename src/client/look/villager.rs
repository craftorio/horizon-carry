//! Villagers: the clothes of their biome, their profession's over them, and a badge of their level.

use super::{Look, Mob, nth};
use crate::prelude::*;

pub(super) fn look(m: &Mob) -> Option<Look> {
    if m.name != "villager" {
        return None;
    }
    let b = m.b;
    let info = read_compound(m.data, "VillagerData").unwrap_or_default();
    let id = |key| read_string(&info, key).map(|v| v.trim_start_matches("minecraft:").to_owned());
    let kind = id("type").unwrap_or_else(|| "plains".into());
    let model = format!("villager{b}");
    let mut look = m.one(model.clone(), format!("entity/villager/villager{b}"));
    if m.baby {
        look.extend(m.one(model, format!("entity/villager/baby/{kind}")));
        return Some(look);
    }
    look.extend(m.one(model.clone(), format!("entity/villager/type/{kind}")));
    if let Some(job) = id("profession").filter(|p| p != "none") {
        look.extend(m.one(model.clone(), format!("entity/villager/profession/{job}")));
        if job != "nitwit" {
            const LEVELS: [&str; 5] = ["stone", "iron", "gold", "emerald", "diamond"];
            let level = int(&info, "level").unwrap_or(1).clamp(1, 5) - 1;
            look.extend(m.one(
                model,
                format!("entity/villager/profession_level/{}", nth(&LEVELS, Some(level))),
            ));
        }
    }
    Some(look)
}
