//! How a carried mob is drawn: the model layers and textures of its type, from its saved data (its variant,
//! whether it is a baby, tamed or shorn). A mod cannot ask the game to draw an entity in a 3D pass, so the
//! rules of each vanilla renderer are written here. `tests.rs` checks them against the game's resources.

use crate::prelude::*;

mod farm;
mod horses;
mod others;
#[cfg(test)]
mod tests;
mod villager;
mod water;

/// The layers to draw: a model layer (`cow`, `sheep#wool`; `#main` when none is named), its texture
/// (`entity/cow/cow_temperate`) and a 0xRRGGBB tint.
pub(crate) type Look = Vec<(String, String, u32)>;

pub(crate) const WHITE: u32 = 0xFFFFFF;

/// The 16 dye colours (`DyeColor.getTextureDiffuseColor`), by their id.
pub(crate) const DYES: [u32; 16] = [
    0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52, 0x9D9D97, 0x169C9C, 0x8932B8,
    0x3C44AA, 0x835432, 0x5E7C16, 0xB02E26, 0x1D1D21,
];

/// A carried mob, as the look rules read it.
pub(crate) struct Mob<'a> {
    /// Its type without the namespace: `cow`.
    pub(crate) name: &'a str,
    pub(crate) data: &'a Compound,
    pub(crate) baby: bool,
    /// `_baby` for a baby, else nothing: the suffix of baby models and textures.
    pub(crate) b: &'static str,
    /// Its `variant` without the namespace (`warm`, `ashen`), where its type keeps one by name.
    variant: Option<String>,
}

impl Mob<'_> {
    pub(crate) fn v(&self) -> Option<&str> {
        self.variant.as_deref()
    }

    /// One layer of a white tint.
    pub(crate) fn one(&self, layer: impl Into<String>, texture: impl Into<String>) -> Look {
        vec![(layer.into(), texture.into(), WHITE)]
    }
}

/// The dye colour with id `id`, white for an unknown one.
pub(crate) fn dye(id: i64) -> u32 {
    usize::try_from(id)
        .ok()
        .and_then(|i| DYES.get(i))
        .copied()
        .unwrap_or(WHITE)
}

/// The name at `index` of `names`, else the first one.
pub(crate) fn nth<'a>(names: &[&'a str], index: Option<i32>) -> &'a str {
    index
        .and_then(|i| usize::try_from(i).ok())
        .and_then(|i| names.get(i))
        .copied()
        .unwrap_or(names[0])
}

/// What a mob of `name` with saved `data` is drawn with.
pub(crate) fn mob_look(name: &str, data: &Compound) -> Look {
    let baby = matches!(data.get(&"Age".into()), Some(Tag::Int(age)) if *age < 0);
    let variant = read_string(data, "variant").map(|v| v.trim_start_matches("minecraft:").to_owned());
    let mob = Mob {
        name,
        data,
        baby,
        b: if baby { "_baby" } else { "" },
        variant,
    };
    farm::look(&mob)
        .or_else(|| horses::look(&mob))
        .or_else(|| water::look(&mob))
        .or_else(|| villager::look(&mob))
        .or_else(|| others::look(&mob))
        // most types: the model of their name, the texture of their name in a folder of their name
        .unwrap_or_else(|| mob.one(format!("{name}{}", mob.b), format!("entity/{name}/{name}{}", mob.b)))
}
