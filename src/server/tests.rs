//! The pick-up and put-down rules in a world in memory.

use super::logic::{click_block, click_mob, death, put_at};
use super::world::{BlockFacts, MobFacts, PlayerFacts, World};
use crate::config::{DEFAULT, Rules};
use crate::prelude::*;

const ME: u128 = 1;
const OTHER: u128 = 2;
const UP: [i32; 3] = [0, 1, 0];

#[derive(Default)]
struct Memory {
    players: HashMap<u128, PlayerFacts>,
    blocks: HashMap<[i32; 3], BlockFacts>,
    mobs: HashMap<u32, MobFacts>,
    carried: HashMap<u128, Carried>,
    summoned: Vec<(String, [f64; 3], Compound)>,
}

impl World for Memory {
    fn player(&self, uuid: u128) -> Option<PlayerFacts> {
        self.players.get(&uuid).cloned()
    }
    fn block(&self, pos: [i32; 3]) -> Option<BlockFacts> {
        Some(
            self.blocks
                .get(&pos)
                .cloned()
                .unwrap_or_else(|| block("minecraft:air", false)),
        )
    }
    fn remove_block(&mut self, pos: [i32; 3]) -> bool {
        self.blocks.remove(&pos).is_some()
    }
    fn place_block(&mut self, pos: [i32; 3], state: &str, data: Option<&Compound>) -> bool {
        let mut placed = block(state, data.is_some());
        placed.data = data.cloned();
        self.blocks.insert(pos, placed);
        true
    }
    fn mob(&mut self, id: u32) -> Option<MobFacts> {
        self.mobs.get(&id).cloned()
    }
    fn remove_mob(&mut self, id: u32) -> bool {
        self.mobs.remove(&id).is_some()
    }
    fn room(&self, feet: [f64; 3], [width, height]: [f64; 2]) -> bool {
        let (half, e) = (width / 2.0, 1e-3);
        let (lo, hi) = (
            [feet[0] - half + e, feet[1] + e, feet[2] - half + e],
            [feet[0] + half - e, feet[1] + height - e, feet[2] + half - e],
        );
        let span = |a: usize| (lo[a].floor() as i32)..=(hi[a].floor() as i32);
        span(0).all(|x| span(1).all(|y| span(2).all(|z| self.block([x, y, z]).is_some_and(|b| b.replaceable))))
    }
    fn summon(&mut self, kind: &str, feet: [f64; 3], data: &Compound) -> bool {
        self.summoned.push((kind.to_owned(), feet, data.clone()));
        true
    }
    fn carried(&mut self, uuid: u128) -> Option<Carried> {
        self.carried.get(&uuid).cloned()
    }
    fn set_carried(&mut self, uuid: u128, what: Option<Carried>) {
        match what {
            Some(what) => self.carried.insert(uuid, what),
            None => self.carried.remove(&uuid),
        };
    }
}

/// A block of `state`; air, grass and water are replaceable, a block with contents is a chest's kind.
fn block(state: &str, has_contents: bool) -> BlockFacts {
    let id = block_id(state);
    BlockFacts {
        state: state.to_owned(),
        data: None,
        has_contents,
        breakable: id != "minecraft:bedrock",
        replaceable: matches!(id, "minecraft:air" | "minecraft:short_grass" | "minecraft:water"),
        viewed: false,
    }
}

fn items() -> Compound {
    [(
        "Items".into(),
        Tag::List(vec![Tag::Compound([("id".into(), string("minecraft:diamond"))].into())]),
    )]
    .into()
}

/// A sneaking player with both hands empty in survival, standing at the origin and looking north; a chest with a
/// diamond at [2, 0, 0], on stone.
fn world() -> Memory {
    let mut w = Memory::default();
    let me = PlayerFacts {
        sneaking: true,
        hands_empty: true,
        may_build: true,
        feet: [0, 0, 0],
        facing: Some("north".into()),
    };
    w.players.insert(ME, me);
    let mut chest = block("minecraft:chest[facing=east,type=left,waterlogged=false]", true);
    chest.data = Some(items());
    w.blocks.insert([2, 0, 0], chest);
    for x in -3..=5 {
        for z in -3..=3 {
            w.blocks.insert([x, -1, z], block("minecraft:stone", false));
        }
    }
    w
}

fn cow(data: Compound) -> MobFacts {
    MobFacts {
        kind: "minecraft:cow".into(),
        data,
        size: [0.9, 1.4],
        living: true,
        monster: false,
        alone: true,
    }
}

fn rules() -> Rules {
    Rules::parse(DEFAULT).unwrap()
}

#[test]
fn a_chest_is_picked_up_with_its_items() {
    let mut w = world();
    assert_eq!(
        click_block(&mut w, &rules(), ME, [2, 0, 0], UP),
        Some(InteractionResult::Success)
    );
    assert!(!w.blocks.contains_key(&[2, 0, 0]));
    let Some(Carried::Block { state, data }) = w.carried.get(&ME) else {
        panic!("nothing carried")
    };
    assert_eq!(state, "minecraft:chest[facing=east,type=left,waterlogged=false]");
    assert_eq!(data.as_ref(), Some(&items()));
}

#[test]
fn only_a_sneaking_player_with_empty_hands_who_may_build_picks_up() {
    for change in [
        |p: &mut PlayerFacts| p.sneaking = false,
        |p: &mut PlayerFacts| p.hands_empty = false,
        |p: &mut PlayerFacts| p.may_build = false,
    ] {
        let mut w = world();
        change(w.players.get_mut(&ME).unwrap());
        assert_eq!(click_block(&mut w, &rules(), ME, [2, 0, 0], UP), None);
        assert!(w.blocks.contains_key(&[2, 0, 0]) && w.carried.is_empty());
    }
}

#[test]
fn the_rules_decide_which_blocks() {
    let mut w = world();
    w.blocks.insert([3, 0, 0], block("minecraft:stone", false));
    w.blocks.insert([4, 0, 0], block("minecraft:spawner", true));
    w.blocks.insert([5, 0, 0], block("minecraft:red_bed[part=foot]", true));
    for pos in [[3, 0, 0], [4, 0, 0], [5, 0, 0]] {
        assert_eq!(click_block(&mut w, &rules(), ME, pos, UP), None, "{pos:?}");
    }
    // with any block allowed, stone comes up too
    let any = Rules::parse("[blocks]\nonly_with_contents = false\n").unwrap();
    assert_eq!(
        click_block(&mut w, &any, ME, [3, 0, 0], UP),
        Some(InteractionResult::Success)
    );
}

#[test]
fn a_chest_that_someone_looks_into_or_bedrock_stays() {
    let mut w = world();
    w.blocks.get_mut(&[2, 0, 0]).unwrap().viewed = true;
    assert_eq!(
        click_block(&mut w, &rules(), ME, [2, 0, 0], UP),
        Some(InteractionResult::Fail)
    );
    assert!(w.blocks.contains_key(&[2, 0, 0]) && w.carried.is_empty());
    let any = Rules::parse("[blocks]\nonly_with_contents = false\n").unwrap();
    assert_eq!(
        click_block(&mut w, &any, ME, [0, -1, 0], UP),
        Some(InteractionResult::Success)
    );
    w.blocks.insert([1, -1, 0], block("minecraft:bedrock", false));
    w.carried.clear();
    assert_eq!(click_block(&mut w, &any, ME, [1, -1, 0], UP), None);
}

#[test]
fn a_carried_chest_goes_down_whole_facing_the_player() {
    let mut w = world();
    click_block(&mut w, &rules(), ME, [2, 0, 0], UP);
    // a click on the top of the stone at [3, -1, 0] puts it on that stone
    assert_eq!(
        click_block(&mut w, &rules(), ME, [3, -1, 0], UP),
        Some(InteractionResult::Success)
    );
    let chest = &w.blocks[&[3, 0, 0]];
    // the half of a double chest comes down single, its front to the player (who looks north)
    assert_eq!(
        chest.state,
        "minecraft:chest[facing=south,type=single,waterlogged=false]"
    );
    assert_eq!(chest.data.as_ref(), Some(&items()));
    assert!(w.carried.is_empty());
}

#[test]
fn a_click_on_grass_puts_it_in_the_grass_s_place() {
    let mut w = world();
    click_block(&mut w, &rules(), ME, [2, 0, 0], UP);
    w.blocks.insert([0, 0, 2], block("minecraft:short_grass", false));
    assert_eq!(
        click_block(&mut w, &rules(), ME, [0, 0, 2], UP),
        Some(InteractionResult::Success)
    );
    assert!(block_id(&w.blocks[&[0, 0, 2]].state) == "minecraft:chest");
}

#[test]
fn nothing_goes_down_where_the_player_stands_or_where_no_room_is() {
    let mut w = world();
    click_block(&mut w, &rules(), ME, [2, 0, 0], UP);
    // on the stone under the player's feet: the spot is their feet
    assert_eq!(
        click_block(&mut w, &rules(), ME, [0, -1, 0], UP),
        Some(InteractionResult::Fail)
    );
    // on the side of a stone with stone next to it: no spot
    w.blocks.insert([3, 0, 0], block("minecraft:stone", false));
    w.blocks.insert([4, 0, 0], block("minecraft:stone", false));
    assert_eq!(
        click_block(&mut w, &rules(), ME, [3, 0, 0], [1, 0, 0]),
        Some(InteractionResult::Fail)
    );
    assert!(w.carried.contains_key(&ME));
}

#[test]
fn a_cow_is_carried_without_its_old_place() {
    let mut w = world();
    let data: Compound = [
        ("UUID".into(), Tag::IntArray(vec![1, 2, 3, 4])),
        ("Health".into(), Tag::Float(10f32.to_bits())),
    ]
    .into();
    w.mobs.insert(7, cow(data));
    assert_eq!(click_mob(&mut w, &rules(), ME, 7), Some(InteractionResult::Success));
    assert!(w.mobs.is_empty());
    let Some(Carried::Entity { kind, data, size }) = w.carried.get(&ME) else {
        panic!("nothing carried")
    };
    assert_eq!((kind.as_str(), *size), ("minecraft:cow", [0.9, 1.4]));
    assert!(data.contains_key(&"Health".into()) && !data.contains_key(&"UUID".into()));
    // hands full: a second mob is not picked up, and nothing else is done with it
    w.mobs.insert(8, cow(Compound::new()));
    assert_eq!(click_mob(&mut w, &rules(), ME, 8), Some(InteractionResult::Fail));
    assert!(w.mobs.contains_key(&8));
}

#[test]
fn the_rules_decide_which_mobs() {
    let ints = |uuid: u128| {
        (0..4)
            .rev()
            .map(|i| (uuid >> (32 * i)) as u32 as i32)
            .collect::<Vec<_>>()
    };
    let mut w = world();
    w.mobs.insert(
        1,
        MobFacts {
            kind: "minecraft:zombie".into(),
            monster: true,
            ..cow(Compound::new())
        },
    );
    w.mobs.insert(
        2,
        MobFacts {
            alone: false,
            ..cow(Compound::new())
        },
    );
    w.mobs
        .insert(3, cow([("Owner".into(), Tag::IntArray(ints(OTHER)))].into()));
    for id in [1, 2, 3] {
        assert_eq!(click_mob(&mut w, &rules(), ME, id), None, "mob {id}");
    }
    let mut mine = world();
    mine.mobs
        .insert(4, cow([("Owner".into(), Tag::IntArray(ints(ME)))].into()));
    assert_eq!(click_mob(&mut mine, &rules(), ME, 4), Some(InteractionResult::Success));
    let monsters = Rules::parse("[mobs]\nmonsters = true\n").unwrap();
    assert_eq!(click_mob(&mut w, &monsters, ME, 1), Some(InteractionResult::Success));
}

#[test]
fn a_cow_needs_room_for_its_body() {
    let mut w = world();
    w.carried.insert(
        ME,
        Carried::Entity {
            kind: "minecraft:cow".into(),
            data: Compound::new(),
            size: [0.9, 1.4],
        },
    );
    // a gap one block high: a stone over the spot
    w.blocks.insert([3, 1, 0], block("minecraft:stone", false));
    assert!(!put_at(&mut w, ME, [3, 0, 0]));
    assert!(w.summoned.is_empty() && w.carried.contains_key(&ME));
    // two blocks high, one wide: the cow (0.9 wide) fits
    w.blocks.insert([0, 0, 2], block("minecraft:stone", false));
    w.blocks.insert([2, 0, 2], block("minecraft:stone", false));
    assert!(put_at(&mut w, ME, [1, 0, 2]));
    assert_eq!(w.summoned[0].1, [1.5, 0.0, 2.5]);
    assert!(w.carried.is_empty());
}

#[test]
fn a_dead_player_s_load_stays_where_they_died() {
    let mut w = world();
    click_block(&mut w, &rules(), ME, [2, 0, 0], UP);
    assert!(death(&mut w, ME, [0, 0, 0]));
    assert_eq!(block_id(&w.blocks[&[0, 0, 0]].state), "minecraft:chest");
    // a cow, where the feet's block is taken: the first place above with room
    let mut w = world();
    w.carried.insert(
        ME,
        Carried::Entity {
            kind: "minecraft:cow".into(),
            data: Compound::new(),
            size: [0.9, 1.4],
        },
    );
    w.blocks.insert([0, 0, 0], block("minecraft:stone", false));
    w.blocks.insert([0, 2, 0], block("minecraft:stone", false));
    assert!(death(&mut w, ME, [0, 0, 0]));
    assert_eq!(w.summoned[0].1, [0.5, 3.0, 0.5]);
    assert!(!death(&mut w, ME, [0, 0, 0]), "nothing is left to put down");
}
