//! The arms of this client's own player in first person: the arm parts of the player model. The hand hook draws
//! them along the sides of the load, with the skin that the game gives for the player's arm. In third person the
//! arms go forward through JEA (`server::arms`), so the model draws them with its armour.

use crate::prelude::*;

/// The arm parts of a player's model (wide or slim, as their skin).
#[derive(Clone)]
pub(crate) struct Arms {
    pub(crate) right: Arc<ModelPart>,
    pub(crate) left: Arc<ModelPart>,
}

pub(crate) fn arms_of(
    host: &dyn ClientHost,
    layers: &mut HashMap<String, Option<Arc<ModelPart>>>,
    player: PlayerRef,
) -> Option<Arms> {
    let slim = host.player_skin(player.entity()).is_some_and(|skin| skin.slim);
    let model = layer(host, layers, if slim { "player_slim" } else { "player" })?;
    let part = |name: &str| model.children().find(|(n, _)| *n == name).map(|(_, part)| part.clone());
    Some(Arms {
        right: Arc::new(part("right_arm")?),
        left: Arc::new(part("left_arm")?),
    })
}
