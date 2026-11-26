//! The hooks' way into the rules: the game's world of the acting player, and the rules in force.

use super::game::GameWorld;
use super::logic;
use crate::prelude::*;

/// The server's answer to a right click on a block, or None to leave the click alone.
pub(crate) fn server_click(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let mut world = GameWorld::of(host, click.player)?;
    logic::click_block(&mut world, &config::rules(), click.player, click.pos, click.hit.face)
}

/// The server's answer to a right click on a mob, or None to leave the click alone.
pub(crate) fn server_pick_mob(host: &mut dyn ServerHost, event: &EntityInteractEvent<'_>) -> Option<InteractionResult> {
    let player = host.players().acting_player()?.uuid();
    let mut world = GameWorld::of(host, player)?;
    logic::click_mob(&mut world, &config::rules(), player, u32::try_from(event.entity).ok()?)
}

/// Whether the player whose action a server hook hears carries something.
pub(crate) fn acting_carries(host: &mut dyn ServerHost) -> bool {
    host.players()
        .acting_player()
        .is_some_and(|player| carried(host, player.uuid()).is_some())
}
