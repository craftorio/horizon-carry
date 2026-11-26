//! Putting down into water. The game does not aim at water: a click there with empty hands is a click at
//! nothing, and the client sends nothing for it. So the client of a carrying player sends `PutDown` on such a
//! click, and the server walks along the player's view to find the water.

use crate::prelude::*;

/// "Put what I carry down where I look": the client's message, on a click at nothing.
#[derive(PayloadCodec)]
pub(crate) struct PutDown {
    /// Always 0: a field for a later version.
    pub(crate) version: VarInt,
}

pub(crate) const PUT_DOWN: &str = "craftorio_carry:put_down";

/// How far a player reaches, in blocks, and the step of the walk along their view.
const REACH: f64 = 5.0;
const STEP: f64 = 0.05;

/// The first water block along the view of `player`, before any block that is not replaceable.
fn water_in_view(host: &dyn Host, level: LevelRef, player: PlayerRef) -> Option<[i32; 3]> {
    let entity = player.entity();
    let (eye, look) = (entity.eye_position(host)?, entity.view_vector(host)?);
    let mut last = None;
    for i in 1..=(REACH / STEP) as i32 {
        let t = f64::from(i) * STEP;
        let pos = [0, 1, 2].map(|a| (eye[a] + look[a] * t).floor() as i32);
        if last == Some(pos) {
            continue;
        }
        last = Some(pos);
        let state = level.block_state(host, pos)?;
        if host.block_states().replaceable(state) != Some(true) {
            return None;
        }
        let text = host.block_state_string(state)?;
        if matches!(block_id(&text), "minecraft:water" | "minecraft:bubble_column") {
            return Some(pos);
        }
    }
    None
}

/// The server's answer to `PutDown` from `player`: what they carry goes into the water they look at.
pub(crate) fn put_down_in_water(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(level) = player.level(host) else { return };
    let Some(spot) = water_in_view(host, level, player) else {
        return;
    };
    if let Some(mut world) = super::game::GameWorld::of(host, player.uuid()) {
        super::logic::put_at(&mut world, player.uuid(), spot);
    }
}

/// The message's token on the client, from `register`.
static TO_SERVER: std::sync::OnceLock<ToServer<PutDown>> = std::sync::OnceLock::new();

/// Declares `PutDown`: the server puts down on it, the client keeps the token to send it.
pub(crate) fn register(registrar: &mut dyn Registrar) {
    let mut payloads = registrar.payload_handlers();
    let mut view = payloads.registrar("1");
    let token = view.play_to_server_typed::<PutDown>(PayloadId(PUT_DOWN.into()), |_, cx| {
        let player = cx.player().map_err(|e| format!("{e:?}"))?;
        put_down_in_water(cx.server_host().map_err(|e| format!("{e:?}"))?, player);
        Ok(())
    });
    let _ = TO_SERVER.set(token);
}

/// On the client: a click at nothing while carrying asks the server to put down into water.
pub(crate) fn click_at_nothing(host: &mut dyn ClientHost) {
    if client_carrying(host)
        && let Some(token) = TO_SERVER.get()
    {
        let _ = token.send(host, &PutDown { version: VarInt(0) });
    }
}
