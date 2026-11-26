//! Picking up: who may, what may be picked up, and the server's answer to a click on a block or a mob.

use crate::prelude::*;

/// Whether a player picks this block up: it can be broken and the rules let it ([`config::Rules::block`]).
pub(crate) fn carriable_block(host: &dyn Host, state: BlockState) -> bool {
    let Some(text) = host.block_state_string(state) else {
        return false;
    };
    let facts = host.block_states();
    facts.destroy_speed(state).is_some_and(|speed| speed >= 0.0)
        && config::rules().block(block_id(&text), facts.has_block_entity(state) == Some(true))
}

/// Whether a player picks this mob up: a living one, alone (no rider, not riding), in one piece, and one the
/// rules let ([`config::Rules::mob`]).
pub(crate) fn carriable_mob(host: &dyn Host, entity: EntityRef, kind: &str) -> bool {
    config::rules().mob(kind, host.entity_type_is_enemy(kind))
        && host.living(entity).is_some()
        && entity.is_multipart(host) != Some(true)
        && entity.passengers(host).is_none_or(|riders| riders.is_empty())
        && entity.vehicle(host).is_none()
}

/// Sneaking, both hands empty, and not a spectator or in adventure mode.
pub(crate) fn may_pick_up(host: &dyn Host, player: PlayerRef) -> bool {
    let entity = player.entity();
    let Some(living) = host.living(entity) else {
        return false;
    };
    let empty = |hand| {
        living
            .hand_item(host, hand)
            .is_none_or(|stack: ItemStack| stack.count() == 0)
    };
    entity.is_shift_key_down(host) == Some(true)
        && empty(Hand::Main)
        && empty(Hand::Off)
        && matches!(player.game_mode(host), Some(GameMode::Survival | GameMode::Creative))
}

/// The server's answer to a right click on a block, or None to leave the click alone.
pub(crate) fn server_click(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let player = host.player(click.player)?;
    let level = player.level(host)?;
    if let Some(what) = carried(host, click.player) {
        let placed = target(host, level, click)
            .filter(|&pos| !inside_player(host, player, pos))
            .is_some_and(|pos| {
                let dimension = dimension_of(host, player).unwrap_or_default();
                let facing = player.entity().direction(host);
                put_down(host, level, &dimension, pos, &what, facing)
            });
        if placed {
            let _ = CARRIED.set(host, Holder::Player(click.player), None);
        }
        return Some(if placed {
            InteractionResult::Success
        } else {
            InteractionResult::Fail
        });
    }
    if !may_pick_up(host, player) {
        return None;
    }
    let state = level.block_state(host, click.pos)?;
    if !carriable_block(host, state) {
        return None;
    }
    let block_entity = host.block_entity(level, click.pos);
    // someone looking into it could still take what is inside
    if block_entity
        .and_then(|entity| entity.viewers(host))
        .is_some_and(|viewers| !viewers.is_empty())
    {
        return Some(InteractionResult::Fail);
    }
    let text = host.block_state_string(state)?;
    let data = block_entity.and_then(|entity| entity.data(host));
    let air = host.block_state_id("minecraft:air")?;
    let flags =
        BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS | BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDE_EFFECTS;
    if !level.set_block(host, click.pos, air, flags).unwrap_or(false) {
        return Some(InteractionResult::Fail);
    }
    let _ = CARRIED.set(
        host,
        Holder::Player(click.player),
        Some(Carried::Block { state: text, data }),
    );
    Some(InteractionResult::Success)
}

/// Whether the player whose action a server hook hears carries something.
pub(crate) fn acting_carries(host: &mut dyn ServerHost) -> bool {
    host.players()
        .acting_player()
        .is_some_and(|player| carried(host, player.uuid()).is_some())
}

pub(crate) fn server_pick_mob(host: &mut dyn ServerHost, event: &EntityInteractEvent<'_>) -> Option<InteractionResult> {
    let player = host.players().acting_player()?;
    let uuid = player.uuid();
    if carried(host, uuid).is_some() {
        // hands are full: no trading, feeding or leashing
        return Some(InteractionResult::Fail);
    }
    if !may_pick_up(host, player) {
        return None;
    }
    let entity = host.entity(u32::try_from(event.entity).ok()?)?;
    if !carriable_mob(host, entity, event.kind) {
        return None;
    }
    let mut data = entity.nbt(host)?;
    // somebody else's pet stays theirs
    if let Some(Tag::IntArray(owner)) = data.get(&"Owner".into())
        && uuid_from_ints(owner) != Some(uuid)
        && !config::rules().other_players_pets
    {
        return None;
    }
    for key in ["UUID", "Pos", "Motion", "FallDistance", "fall_distance"] {
        data.remove(&key.into());
    }
    let size = entity.dimensions(host).unwrap_or(DEFAULT_SIZE);
    if !entity.discard(host) {
        return Some(InteractionResult::Fail);
    }
    let _ = CARRIED.set(
        host,
        Holder::Player(uuid),
        Some(Carried::Entity {
            kind: event.kind.to_owned(),
            data,
            size,
        }),
    );
    Some(InteractionResult::Success)
}
