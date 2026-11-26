//! A carrying player walks slower: a movement speed modifier while they carry something.

use crate::prelude::*;

/// Players slowed down by this mod, so the modifier is added and taken away once.
pub(crate) static SLOWED: Mutex<Option<HashSet<u128>>> = Mutex::new(None);

pub(crate) fn slowdown_id() -> Option<Identifier> {
    Identifier::parse(&format!("{NAMESPACE}:carrying")).ok()
}

pub(crate) fn update_slowdown(host: &mut dyn ServerHost, player: PlayerRef) {
    let uuid = player.uuid();
    let carrying = carried(host, uuid).is_some();
    let mut slowed = SLOWED.lock().unwrap_or_else(|poison| poison.into_inner());
    let slowed = slowed.get_or_insert_with(HashSet::new);
    if carrying == slowed.contains(&uuid) {
        return;
    }
    let (Some(living), Some(id)) = (host.living(player.entity()), slowdown_id()) else {
        return;
    };
    if carrying {
        let modifier = AttributeModifier::new(id, -config::rules().slowdown, AttributeOperation::AddMultipliedTotal);
        living.add_attribute_modifier(host, "minecraft:movement_speed", modifier, ModifierLifetime::Transient);
        slowed.insert(uuid);
    } else {
        living.remove_attribute_modifier(host, "minecraft:movement_speed", &id);
        slowed.remove(&uuid);
    }
}

/// A player who leaves loses the modifier (it is transient), so the next tick after they come back adds it again.
pub(crate) fn forget_slowdown(player: u128) {
    if let Some(slowed) = SLOWED.lock().unwrap_or_else(|poison| poison.into_inner()).as_mut() {
        slowed.remove(&player);
    }
}
