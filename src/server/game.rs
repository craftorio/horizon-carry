//! The [`World`] of the game: the level a player is in, through the server host.

use super::world::{BlockFacts, MobFacts, PlayerFacts, World};
use crate::prelude::*;

pub(crate) struct GameWorld<'a> {
    pub(crate) host: &'a mut dyn ServerHost,
    pub(crate) level: LevelRef,
    pub(crate) dimension: String,
}

impl<'a> GameWorld<'a> {
    /// The world of the level `player` is in.
    pub(crate) fn of(host: &'a mut dyn ServerHost, player: u128) -> Option<Self> {
        let player = host.player(player)?;
        let level = player.level(host)?;
        let dimension = player
            .dimension(host)
            .map(|id: Identifier| id.to_string())
            .unwrap_or_default();
        Some(GameWorld { host, level, dimension })
    }
}

impl World for GameWorld<'_> {
    fn player(&self, uuid: u128) -> Option<PlayerFacts> {
        let host: &dyn Host = &*self.host;
        let player = host.player(uuid)?;
        let entity = player.entity();
        let living = host.living(entity)?;
        let empty = |hand| {
            living
                .hand_item(host, hand)
                .is_none_or(|stack: ItemStack| stack.count() == 0)
        };
        Some(PlayerFacts {
            sneaking: entity.is_shift_key_down(host) == Some(true),
            hands_empty: empty(Hand::Main) && empty(Hand::Off),
            may_build: matches!(player.game_mode(host), Some(GameMode::Survival | GameMode::Creative)),
            feet: entity.block_position(host)?,
            facing: entity.direction(host).map(str::to_owned),
        })
    }

    fn block(&self, pos: [i32; 3]) -> Option<BlockFacts> {
        let host: &dyn Host = &*self.host;
        let state = self.level.block_state(host, pos)?;
        let facts = host.block_states();
        let entity = host.block_entity(self.level, pos);
        Some(BlockFacts {
            state: host.block_state_string(state)?,
            data: entity.and_then(|e| e.data(host)),
            has_contents: facts.has_block_entity(state) == Some(true),
            breakable: facts.destroy_speed(state).is_some_and(|speed| speed >= 0.0),
            replaceable: facts.replaceable(state) == Some(true),
            viewed: entity
                .and_then(|e| e.viewers(host))
                .is_some_and(|viewers| !viewers.is_empty()),
        })
    }

    fn remove_block(&mut self, pos: [i32; 3]) -> bool {
        let Some(air) = self.host.block_state_id("minecraft:air") else {
            return false;
        };
        let flags =
            BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS | BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDE_EFFECTS;
        self.level.set_block(self.host, pos, air, flags).unwrap_or(false)
    }

    fn place_block(&mut self, pos: [i32; 3], state: &str, data: Option<&Compound>) -> bool {
        let (host, level) = (&mut *self.host, self.level);
        let Some(block) = host.block_state_id(state) else {
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
            let at = pos.map(|c| f64::from(c) + 0.5);
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

    fn mob(&mut self, id: u32) -> Option<MobFacts> {
        let entity = self.host.entity(id)?;
        let data = entity.nbt(self.host)?;
        let host: &dyn Host = &*self.host;
        let kind = entity.kind(host)?.id(host);
        let alone = entity.is_multipart(host) != Some(true)
            && entity.passengers(host).is_none_or(|riders| riders.is_empty())
            && entity.vehicle(host).is_none();
        Some(MobFacts {
            monster: host.entity_type_is_enemy(&kind),
            living: host.living(entity).is_some(),
            size: entity.dimensions(host).unwrap_or(DEFAULT_SIZE),
            data,
            kind,
            alone,
        })
    }

    fn remove_mob(&mut self, id: u32) -> bool {
        self.host.entity(id).is_some_and(|entity| entity.discard(self.host))
    }

    fn room(&self, feet: [f64; 3], [width, height]: [f64; 2]) -> bool {
        // a thousandth smaller, so a box that just fills a gap still fits
        let (half, e) = (width / 2.0, 1e-3);
        let aabb = [
            feet[0] - half + e,
            feet[1] + e,
            feet[2] - half + e,
            feet[0] + half - e,
            feet[1] + height - e,
            feet[2] + half - e,
        ];
        self.level.has_collisions(&*self.host, aabb) == Some(false)
    }

    fn summon(&mut self, kind: &str, feet: [f64; 3], data: &Compound) -> bool {
        self.host.summon(&self.dimension, kind, feet, data.clone()).is_some()
    }

    fn carried(&mut self, uuid: u128) -> Option<Carried> {
        carried(self.host, uuid)
    }

    fn set_carried(&mut self, uuid: u128, what: Option<Carried>) {
        let _ = CARRIED.set(self.host, Holder::Player(uuid), what);
    }
}
