//! The arms of a carrying player go forward through Just Enough Animations (JEA, the mod `jea`): the server plays
//! the clip `craftorio_carry:hold` (`assets/craftorio_carry/animations/hold.animation.json`) on the player while
//! they carry something, and stops it when they do not. A WebAssembly mod cannot call JEA's Rust API, so this runs
//! JEA's command with `run_command` (the console, level 4). Every client with JEA sees the arms, the armour on them
//! too. Without JEA on the server, nothing plays and the arms hang.

use crate::prelude::*;

/// The clip and its channel. Priority 1500 is in JEA's band of side states (1000-1999): a Horizon emote (2500) and
/// Knockout (3000) play over it. Replace: the arms keep the clip's angles while the player walks; a crouch still
/// moves them down with the body, because the clip has no position. Fades of 3 ticks, in and out.
const PLAY: &str = "craftorio_carry:hold craftorio_carry:main 1500 3 loop";
const CHANNEL: &str = "craftorio_carry:main";

/// What this server last told JEA for each player: true for "play". A player not in it (after a login or a
/// restart) may still have the old animation in JEA's saved attachment, so the first tick tells JEA again.
static TOLD: Mutex<Option<HashMap<u128, bool>>> = Mutex::new(None);

/// Whether the server has JEA's command, asked once.
static HAS_JEA: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// A UUID as a command reads it: `0123abcd-...`.
pub(crate) fn uuid_text(uuid: u128) -> String {
    let h = format!("{uuid:032x}");
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// The command for a player who carries (`carrying`), when JEA was `told` something else before (None: nothing
/// since the login). None when JEA already has it.
pub(crate) fn command(uuid: u128, told: Option<bool>, carrying: bool) -> Option<String> {
    if told == Some(carrying) {
        return None;
    }
    let who = uuid_text(uuid);
    Some(if carrying {
        format!("jea play {who} {PLAY}")
    } else {
        format!("jea stop {who} {CHANNEL} 3")
    })
}

/// Once a tick for each player: tells JEA when the player begins or ends to carry.
pub(crate) fn update_arms(host: &mut dyn ServerHost, player: PlayerRef) {
    let uuid = player.uuid();
    let carrying = carried(host, uuid).is_some();
    let mut told = TOLD.lock().unwrap_or_else(|poison| poison.into_inner());
    let told = told.get_or_insert_with(HashMap::new);
    let Some(line) = command(uuid, told.get(&uuid).copied(), carrying) else {
        return;
    };
    told.insert(uuid, carrying);
    if !*HAS_JEA.get_or_init(|| host.commands().command_info("jea").is_some()) {
        return;
    }
    if !host.commands().run_command(&line) {
        let text = format!("JEA did not run `{line}`");
        let _ = host.messages().console_log(NAMESPACE, ConsoleLevel::Warn, &text);
    }
}

/// A player who leaves: their next login tells JEA again.
pub(crate) fn forget_arms(player: u128) {
    if let Some(told) = TOLD.lock().unwrap_or_else(|poison| poison.into_inner()).as_mut() {
        told.remove(&player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jea_hears_each_change_once() {
        let uuid = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210;
        assert_eq!(uuid_text(uuid), "01234567-89ab-cdef-fedc-ba9876543210");
        assert_eq!(
            command(uuid, Some(false), true).as_deref(),
            Some("jea play 01234567-89ab-cdef-fedc-ba9876543210 craftorio_carry:hold craftorio_carry:main 1500 3 loop")
        );
        assert_eq!(command(uuid, Some(true), true), None);
        assert_eq!(
            command(uuid, Some(true), false).as_deref(),
            Some("jea stop 01234567-89ab-cdef-fedc-ba9876543210 craftorio_carry:main 3")
        );
        assert_eq!(command(uuid, Some(false), false), None);
        // after a login, JEA's saved animation is stopped or played again
        assert!(command(uuid, None, false).unwrap().starts_with("jea stop"));
        assert!(command(uuid, None, true).unwrap().starts_with("jea play"));
    }
}
