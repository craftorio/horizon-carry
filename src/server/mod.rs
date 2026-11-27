//! The server: it decides every pick-up and put-down, keeps the hands of a carrying player full, puts down what a
//! dying player carries, and holds the arms of a carrying player forward through JEA (`arms.rs`).

use crate::prelude::*;

pub(crate) mod arms;
pub(crate) mod game;
pub(crate) mod logic;
pub(crate) mod pick;
pub(crate) mod slowdown;
#[cfg(test)]
mod tests;
pub(crate) mod water;
pub(crate) mod world;

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
    let (Some(uuid), Some(feet)) = (entity.uuid(host), entity.block_position(host)) else {
        return;
    };
    if let Some(mut world) = game::GameWorld::of(host, uuid) {
        logic::death(&mut world, uuid, feet);
    }
}
