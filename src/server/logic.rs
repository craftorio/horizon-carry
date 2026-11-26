//! The rules of picking up and putting down, over a [`World`].

use super::world::World;
use crate::config::Rules;
use crate::prelude::*;

/// The keys of a mob's data that belong to its old place: they are not carried.
const PLACE_KEYS: [&str; 5] = ["UUID", "Pos", "Motion", "FallDistance", "fall_distance"];

/// A right click of `player` on the block at `pos`, on its `face` (a unit vector): what they carry goes down
/// there, else the block comes up. None leaves the click to the game.
pub(crate) fn click_block(
    w: &mut dyn World,
    rules: &Rules,
    player: u128,
    pos: [i32; 3],
    face: [i32; 3],
) -> Option<InteractionResult> {
    if w.carried(player).is_some() {
        // the clicked block itself when it is replaceable (grass), else the block next to the clicked face
        let next = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
        let spot = [pos, next]
            .into_iter()
            .find(|&p| w.block(p).is_some_and(|b| b.replaceable));
        let placed = spot.is_some_and(|spot| put_at(w, player, spot));
        return Some(if placed {
            InteractionResult::Success
        } else {
            InteractionResult::Fail
        });
    }
    pick_block(w, rules, player, pos)
}

/// Whether `player` may pick anything up now: sneaking, both hands empty, in survival or creative.
fn may_pick_up(w: &dyn World, player: u128) -> bool {
    w.player(player)
        .is_some_and(|p| p.sneaking && p.hands_empty && p.may_build)
}

fn pick_block(w: &mut dyn World, rules: &Rules, player: u128, pos: [i32; 3]) -> Option<InteractionResult> {
    if !may_pick_up(w, player) {
        return None;
    }
    let block = w.block(pos)?;
    if !block.breakable || !rules.block(block_id(&block.state), block.has_contents) {
        return None;
    }
    if block.viewed || !w.remove_block(pos) {
        return Some(InteractionResult::Fail);
    }
    w.set_carried(
        player,
        Some(Carried::Block {
            state: block.state,
            data: block.data,
        }),
    );
    Some(InteractionResult::Success)
}

/// A right click of `player` on mob `id`. None leaves the click to the game.
pub(crate) fn click_mob(w: &mut dyn World, rules: &Rules, player: u128, id: u32) -> Option<InteractionResult> {
    if w.carried(player).is_some() {
        // hands full: no trading, feeding or leashing
        return Some(InteractionResult::Fail);
    }
    if !may_pick_up(w, player) {
        return None;
    }
    let mob = w.mob(id)?;
    if !mob.living || !mob.alone || !rules.mob(&mob.kind, mob.monster) {
        return None;
    }
    // somebody else's pet stays theirs
    if let Some(Tag::IntArray(owner)) = mob.data.get(&"Owner".into())
        && uuid_from_ints(owner) != Some(player)
        && !rules.other_players_pets
    {
        return None;
    }
    let mut data = mob.data;
    for key in PLACE_KEYS {
        data.remove(&key.into());
    }
    if !w.remove_mob(id) {
        return Some(InteractionResult::Fail);
    }
    w.set_carried(
        player,
        Some(Carried::Entity {
            kind: mob.kind,
            data,
            size: mob.size,
        }),
    );
    Some(InteractionResult::Success)
}

/// Puts `what` down at `spot`: a block facing the player (`facing`), a mob only where its body has room.
fn place(w: &mut dyn World, what: &Carried, spot: [i32; 3], facing: Option<&str>) -> bool {
    match what {
        Carried::Block { state, data } => w.place_block(spot, &placed_state(state, facing), data.as_ref()),
        Carried::Entity { kind, data, size } => {
            let feet = [f64::from(spot[0]) + 0.5, f64::from(spot[1]), f64::from(spot[2]) + 0.5];
            w.room(feet, *size) && w.summon(kind, feet, data)
        }
    }
}

/// Puts what `player` carries down at `spot`. False (and they still carry it) when it does not go there: the
/// spot is where they stand, or a mob has no room for its body.
pub(crate) fn put_at(w: &mut dyn World, player: u128, spot: [i32; 3]) -> bool {
    let (Some(what), Some(facts)) = (w.carried(player), w.player(player)) else {
        return false;
    };
    let [x, y, z] = facts.feet;
    if spot[0] == x && spot[2] == z && (spot[1] == y || spot[1] == y + 1) {
        return false;
    }
    let placed = place(w, &what, spot, facts.facing.as_deref());
    if placed {
        w.set_carried(player, None);
    }
    placed
}

/// A player died at `feet`: what they carry goes down there, or in the first free place up to 7 blocks above.
/// (The place of a dead player's feet is free to use.)
pub(crate) fn death(w: &mut dyn World, player: u128, feet: [i32; 3]) -> bool {
    let Some(what) = w.carried(player) else { return false };
    let [x, y, z] = feet;
    let placed = (0..8)
        .map(|up| [x, y + up, z])
        .any(|spot| w.block(spot).is_some_and(|b| b.replaceable) && place(w, &what, spot, None));
    if placed {
        w.set_carried(player, None);
    }
    placed
}
