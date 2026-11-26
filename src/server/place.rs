//! Putting down: where a click puts a block or a mob, and whether there is room for it.

use crate::prelude::*;

/// Where a click on `click.pos` puts a block: the clicked block itself when it is replaceable, else the block
/// next to the clicked face.
pub(crate) fn target(host: &dyn Host, level: LevelRef, click: &BlockClick<'_>) -> Option<[i32; 3]> {
    let replaceable = |pos| {
        level
            .block_state(host, pos)
            .and_then(|state| host.block_states().replaceable(state))
            == Some(true)
    };
    let [x, y, z] = click.pos;
    let [dx, dy, dz] = click.hit.face;
    [click.pos, [x + dx, y + dy, z + dz]]
        .into_iter()
        .find(|&pos| replaceable(pos))
}

/// Whether `pos` is where the player stands (feet or head), so a block put there would trap them.
pub(crate) fn inside_player(host: &dyn Host, player: PlayerRef, pos: [i32; 3]) -> bool {
    player
        .entity()
        .block_position(host)
        .is_some_and(|[x, y, z]| pos[0] == x && pos[2] == z && (pos[1] == y || pos[1] == y + 1))
}

/// Whether a mob `size` (width, height) standing at `feet` (its bottom centre) fits: its box touches no
/// block's collision shape. (A thousandth smaller, so a box exactly filling a gap still fits.)
pub(crate) fn room_for(host: &dyn Host, level: LevelRef, feet: [f64; 3], [width, height]: [f64; 2]) -> bool {
    let (half, e) = (width / 2.0, 1e-3);
    let aabb = [
        feet[0] - half + e,
        feet[1] + e,
        feet[2] - half + e,
        feet[0] + half - e,
        feet[1] + height - e,
        feet[2] + half - e,
    ];
    level.has_collisions(host, aabb) == Some(false)
}

/// Puts `what` down at `pos` in `level` (`dimension`). False when nothing was put down.
pub(crate) fn put_down(
    host: &mut dyn ServerHost,
    level: LevelRef,
    dimension: &str,
    pos: [i32; 3],
    what: &Carried,
    facing: Option<&str>,
) -> bool {
    match what {
        Carried::Block { state, data } => {
            let Some(block) = host.block_state_id(&placed_state(state, facing)) else {
                return false;
            };
            if !level
                .set_block(
                    host,
                    pos,
                    block,
                    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                )
                .unwrap_or(false)
            {
                return false;
            }
            if let Some(data) = data
                && let Some(entity) = host.block_entity(level, pos)
            {
                let _ = entity.replace_data(host, data.clone());
            }
            if let Some(sound) = host.block_states().sound(block) {
                let at = [
                    f64::from(pos[0]) + 0.5,
                    f64::from(pos[1]) + 0.5,
                    f64::from(pos[2]) + 0.5,
                ];
                level.play_sound(
                    host,
                    &sound.place,
                    at,
                    SoundSource::Blocks,
                    (sound.volume + 1.0) / 2.0,
                    sound.pitch * 0.8,
                );
            }
            true
        }
        Carried::Entity { kind, data, size } => {
            let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]), f64::from(pos[2]) + 0.5];
            room_for(host, level, at, *size) && host.summon(dimension, kind, at, data.clone()).is_some()
        }
    }
}

pub(crate) fn dimension_of(host: &dyn Host, player: PlayerRef) -> Option<String> {
    player.dimension(host).map(|id: Identifier| id.to_string())
}
