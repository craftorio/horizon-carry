//! The server: it decides every pick-up and put-down, keeps the hands of a carrying player full, and puts down
//! what a dying player carries.

use crate::prelude::*;

pub(crate) mod pick;
pub(crate) mod place;
pub(crate) mod slowdown;
pub(crate) mod water;

/// Takes this mod's config. A file that does not parse is logged, and the rules from before stay.
pub(crate) fn take_config(host: HostSide, file: &ConfigFile) {
    if file.mod_id != NAMESPACE {
        return;
    }
    if let Err(why) = config::load(&file.text)
        && let HostSide::Server(host) = host
    {
        let line = format!("{}: {why}; the rules from before stay", file.file);
        let _ = host.messages().console_log(NAMESPACE, ConsoleLevel::Warn, &line);
    }
}

/// A player who dies puts down what they carry where they died, or in the first free place above it.
pub(crate) fn death(host: &mut dyn ServerHost, event: &LivingDeathEvent<'_>) {
    if event.kind != "minecraft:player" {
        return;
    }
    let Some(entity) = host.entity(event.entity) else {
        return;
    };
    let Some(uuid) = entity.uuid(host) else { return };
    let Some(what) = carried(host, uuid) else { return };
    let (Some(player), Some(level), Some([x, y, z])) =
        (host.player(uuid), entity.level(host), entity.block_position(host))
    else {
        return;
    };
    let dimension = dimension_of(host, player).unwrap_or_default();
    let free = |host: &dyn Host, pos| {
        level
            .block_state(host, pos)
            .and_then(|s| host.block_states().replaceable(s))
            == Some(true)
    };
    // a mob needs room for its whole body
    let placed = (0..8)
        .map(|up| [x, y + up, z])
        .any(|pos| free(host, pos) && put_down(host, level, &dimension, pos, &what, None));
    if placed {
        let _ = CARRIED.set(host, Holder::Player(uuid), None);
    }
}
