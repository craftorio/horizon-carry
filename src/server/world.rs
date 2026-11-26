//! The world as the pick-up and put-down rules see it. `game.rs` answers it from the game; the tests answer it
//! from a world in memory. So the rules in `logic.rs` are tested without the game.

use crate::prelude::*;

/// A player, as the rules read them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlayerFacts {
    pub(crate) sneaking: bool,
    pub(crate) hands_empty: bool,
    /// In survival or creative: a spectator and adventure mode pick up nothing.
    pub(crate) may_build: bool,
    /// The block of their feet: a block is not put there or above it, where it would trap them.
    pub(crate) feet: [i32; 3],
    /// The way they look (`north`), for the way a block put down faces.
    pub(crate) facing: Option<String>,
}

/// A block, as the rules read it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BlockFacts {
    /// `minecraft:chest[facing=north,type=single]`.
    pub(crate) state: String,
    /// Its block entity's data, when it has one.
    pub(crate) data: Option<Compound>,
    pub(crate) has_contents: bool,
    /// It can be broken (bedrock cannot).
    pub(crate) breakable: bool,
    /// Placing a block here replaces it (air, grass, water).
    pub(crate) replaceable: bool,
    /// A player looks into it now: what is inside could still be taken.
    pub(crate) viewed: bool,
}

/// A mob, as the rules read it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MobFacts {
    pub(crate) kind: String,
    /// Its saved data, without its UUID and place.
    pub(crate) data: Compound,
    /// Width and height in blocks.
    pub(crate) size: [f64; 2],
    pub(crate) living: bool,
    pub(crate) monster: bool,
    /// No rider, not riding, in one piece.
    pub(crate) alone: bool,
}

/// The world of one level, where a player acts.
pub(crate) trait World {
    fn player(&self, uuid: u128) -> Option<PlayerFacts>;
    fn block(&self, pos: [i32; 3]) -> Option<BlockFacts>;
    /// Takes the block away, with nothing dropped and its contents gone with it.
    fn remove_block(&mut self, pos: [i32; 3]) -> bool;
    /// Puts block `state` at `pos` with its block entity's `data`.
    fn place_block(&mut self, pos: [i32; 3], state: &str, data: Option<&Compound>) -> bool;
    fn mob(&mut self, id: u32) -> Option<MobFacts>;
    fn remove_mob(&mut self, id: u32) -> bool;
    /// Whether a box of `size` (width, height) standing at `feet` touches no block.
    fn room(&self, feet: [f64; 3], size: [f64; 2]) -> bool;
    fn summon(&mut self, kind: &str, feet: [f64; 3], data: &Compound) -> bool;
    fn carried(&mut self, uuid: u128) -> Option<Carried>;
    fn set_carried(&mut self, uuid: u128, what: Option<Carried>);
}
