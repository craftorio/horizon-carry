//! Carry: what the Java mod Carry On does, for Horizon. Sneak with both hands empty and right-click a block
//! that holds something (a chest, a furnace, a barrel...) or a peaceful mob: it leaves the world, contents and
//! all, and the player carries it, slowed down. A right click on a block puts it back next to that block (or in
//! its place when it is replaceable, as grass is). Dying puts it down where the player died.
//!
//! The server decides; the client answers the same clicks the same way so it neither opens the chest it is
//! about to lose nor plays a use it would have to take back, and draws what is carried in the hands. What a
//! player carries is an attachment on the player: saved with them, synced to their client.

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, AttackEntityEvent, AttributeModifier,
    AttributeOperation, BlockClick, BlockEntityDrawContext, BlockState, BlockUpdateFlags,
    BreakBlockEvent, ClientHost, ClientTickPostEvent, Compound, Config, ConfigFile, ConfigKind,
    ConfigLoadingEvent, ConfigReloadingEvent, ConsoleLevel, EntityInteractEvent, EntityRef,
    EntityRenderLayer, EntityRenderState, Flow, GameMode, Hand, Holder, Hooks, Host,
    HostBlockStates, HostSide, HumanoidPart, Identifier, InteractionResult, ItemDisplayContext,
    ItemStack, LayerTarget, LevelRef, LivingDeathEvent, ModelPart, ModifierLifetime,
    PlayerLoggedOutEvent, PlayerRef, PlayerSwitchHotbarSlotPreEvent, PlayerTickPostEvent,
    PosedParts, RegisterConfigsEvent, RegisterEntityLayersEvent, RegistrationContext,
    RenderHandEvent, RenderPlayerPreEvent, RightClickBlockEvent, RightClickItemEvent, ServerHost,
    SoundSource, Tag,
};
mod config;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

const NAMESPACE: &str = "craftorio_carry";
#[derive(Clone, Debug, PartialEq)]
enum Carried {
    /// A block state as `minecraft:chest[facing=north,...]` and its block entity's data.
    Block {
        state: String,
        data: Option<Compound>,
    },
    /// An entity type and its saved data (without its UUID and position).
    /// `size`: its width and height in blocks, to find room for it again.
    Entity {
        kind: String,
        data: Compound,
        size: [f64; 2],
    },
}

static CARRIED: AttachmentType<Option<Carried>> = AttachmentType {
    namespace: NAMESPACE,
    name: "carried",
    default: || None,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: true,
};

fn string(text: &str) -> Tag {
    Tag::String(text.into())
}

fn read_string(data: &Compound, key: &str) -> Option<String> {
    match data.get(&key.into()) {
        Some(Tag::String(text)) => Some(text.to_string_lossy()),
        _ => None,
    }
}

fn read_compound(data: &Compound, key: &str) -> Option<Compound> {
    match data.get(&key.into()) {
        Some(Tag::Compound(inner)) => Some(inner.clone()),
        _ => None,
    }
}

fn encode(value: &Option<Carried>) -> Result<Compound, AttachmentError> {
    let mut out = Compound::new();
    match value {
        None => {}
        Some(Carried::Block { state, data }) => {
            out.insert("kind".into(), string("block"));
            out.insert("state".into(), string(state));
            if let Some(data) = data {
                out.insert("data".into(), Tag::Compound(data.clone()));
            }
        }
        Some(Carried::Entity { kind, data, size }) => {
            out.insert("kind".into(), string("entity"));
            out.insert("type".into(), string(kind));
            out.insert("data".into(), Tag::Compound(data.clone()));
            out.insert(
                "size".into(),
                Tag::List(size.iter().map(|v| Tag::Double(v.to_bits())).collect()),
            );
        }
    }
    Ok(out)
}

fn decode(data: &Compound) -> Result<Option<Carried>, AttachmentError> {
    match read_string(data, "kind").as_deref() {
        None => Ok(None),
        Some("block") => {
            let state = read_string(data, "state").ok_or(AttachmentError::Decode)?;
            Ok(Some(Carried::Block {
                state,
                data: read_compound(data, "data"),
            }))
        }
        Some("entity") => {
            let kind = read_string(data, "type").ok_or(AttachmentError::Decode)?;
            let size = match data.get(&"size".into()) {
                Some(Tag::List(list)) => match list.as_slice() {
                    [Tag::Double(w), Tag::Double(h)] => [f64::from_bits(*w), f64::from_bits(*h)],
                    _ => DEFAULT_SIZE,
                },
                _ => DEFAULT_SIZE,
            };
            Ok(Some(Carried::Entity {
                kind,
                data: read_compound(data, "data").unwrap_or_default(),
                size,
            }))
        }
        Some(_) => Err(AttachmentError::Decode),
    }
}

/// `minecraft:chest[facing=north,type=left]` → `minecraft:chest`.
fn block_id(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

/// The value of `property` in a state string.
fn property<'a>(state: &'a str, property: &str) -> Option<&'a str> {
    let inner = state.split_once('[')?.1.strip_suffix(']')?;
    inner.split(',').find_map(|pair| {
        pair.split_once('=')
            .filter(|(key, _)| *key == property)
            .map(|(_, value)| value)
    })
}

/// The state string with `property` set to `value` (unchanged when the state has no such property).
fn with_property(state: &str, property: &str, value: &str) -> String {
    let Some((id, rest)) = state.split_once('[') else {
        return state.to_owned();
    };
    let Some(inner) = rest.strip_suffix(']') else {
        return state.to_owned();
    };
    let pairs: Vec<String> = inner
        .split(',')
        .map(|pair| match pair.split_once('=') {
            Some((key, _)) if key == property => format!("{key}={value}"),
            _ => pair.to_owned(),
        })
        .collect();
    format!("{id}[{}]", pairs.join(","))
}

fn opposite(direction: &str) -> Option<&'static str> {
    Some(match direction {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        _ => return None,
    })
}

/// How a carried block is put down: half of a double chest becomes a single one, and a block that faces
/// sideways faces the player who puts it down.
fn placed_state(state: &str, player_facing: Option<&str>) -> String {
    let mut state = if property(state, "type").is_some_and(|t| t == "left" || t == "right") {
        with_property(state, "type", "single")
    } else {
        state.to_owned()
    };
    if let (Some(current), Some(facing)) =
        (property(&state, "facing"), player_facing.and_then(opposite))
        && opposite(current).is_some()
    {
        state = with_property(&state, "facing", facing);
    }
    state
}

/// Whether a player picks this block up: it can be broken and the rules let it ([`config::Rules::block`]).
fn carriable_block(host: &dyn Host, state: BlockState) -> bool {
    let Some(text) = host.block_state_string(state) else {
        return false;
    };
    let facts = host.block_states();
    facts.destroy_speed(state).is_some_and(|speed| speed >= 0.0)
        && config::rules().block(block_id(&text), facts.has_block_entity(state) == Some(true))
}

/// Whether a player picks this mob up: a living one, alone (no rider, not riding), in one piece, and one the
/// rules let ([`config::Rules::mob`]).
fn carriable_mob(host: &dyn Host, entity: EntityRef, kind: &str) -> bool {
    config::rules().mob(kind, host.entity_type_is_enemy(kind))
        && host.living(entity).is_some()
        && entity.is_multipart(host) != Some(true)
        && entity
            .passengers(host)
            .is_none_or(|riders| riders.is_empty())
        && entity.vehicle(host).is_none()
}

/// Sneaking, both hands empty, and not a spectator or in adventure mode.
fn may_pick_up(host: &dyn Host, player: PlayerRef) -> bool {
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
        && matches!(
            player.game_mode(host),
            Some(GameMode::Survival | GameMode::Creative)
        )
}

fn carried(host: &mut dyn ServerHost, player: u128) -> Option<Carried> {
    CARRIED
        .get_existing(host, Holder::Player(player))
        .ok()
        .flatten()
        .flatten()
}

/// Where a click on `click.pos` puts a block: the clicked block itself when it is replaceable, else the block
/// next to the clicked face.
fn target(host: &dyn Host, level: LevelRef, click: &BlockClick<'_>) -> Option<[i32; 3]> {
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
fn inside_player(host: &dyn Host, player: PlayerRef, pos: [i32; 3]) -> bool {
    player
        .entity()
        .block_position(host)
        .is_some_and(|[x, y, z]| pos[0] == x && pos[2] == z && (pos[1] == y || pos[1] == y + 1))
}

fn uuid_from_ints(ints: &[i32]) -> Option<u128> {
    let [a, b, c, d] = ints else { return None };
    Some(
        [*a, *b, *c, *d]
            .iter()
            .fold(0u128, |uuid, part| (uuid << 32) | u128::from(*part as u32)),
    )
}

/// The size of a mob carried before sizes were kept: a cow's.
const DEFAULT_SIZE: [f64; 2] = [0.9, 1.4];

/// Whether a mob `size` (width, height) standing at `feet` (its bottom centre) fits: its box touches no
/// block's collision shape. (A thousandth smaller, so a box exactly filling a gap still fits.)
fn room_for(host: &dyn Host, level: LevelRef, feet: [f64; 3], [width, height]: [f64; 2]) -> bool {
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
fn put_down(
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
            let at = [
                f64::from(pos[0]) + 0.5,
                f64::from(pos[1]),
                f64::from(pos[2]) + 0.5,
            ];
            room_for(host, level, at, *size)
                && host.summon(dimension, kind, at, data.clone()).is_some()
        }
    }
}

fn dimension_of(host: &dyn Host, player: PlayerRef) -> Option<String> {
    player.dimension(host).map(|id: Identifier| id.to_string())
}

/// The server's answer to a right click on a block, or None to leave the click alone.
fn server_click(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
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
    let flags = BlockUpdateFlags::NEIGHBORS
        | BlockUpdateFlags::CLIENTS
        | BlockUpdateFlags::SKIP_BLOCK_ENTITY_SIDE_EFFECTS;
    if !level
        .set_block(host, click.pos, air, flags)
        .unwrap_or(false)
    {
        return Some(InteractionResult::Fail);
    }
    let _ = CARRIED.set(
        host,
        Holder::Player(click.player),
        Some(Carried::Block { state: text, data }),
    );
    Some(InteractionResult::Success)
}

/// What the client does with a click while its player carries something: nothing of its own (no block's use,
/// no item's), the server puts the thing down. Picking up is the server's alone: the client lets the click go
/// to it as usual (a chest opens only on the server's word), so the rules live on the server only.
fn client_carrying(host: &mut dyn ClientHost) -> bool {
    host.local_player().is_some_and(|player| {
        CARRIED
            .get_existing(host, Holder::Player(player.uuid()))
            .ok()
            .flatten()
            .flatten()
            .is_some()
    })
}

/// Whether the player whose action a server hook hears carries something.
fn acting_carries(host: &mut dyn ServerHost) -> bool {
    host.players()
        .acting_player()
        .is_some_and(|player| carried(host, player.uuid()).is_some())
}

fn server_pick_mob(
    host: &mut dyn ServerHost,
    event: &EntityInteractEvent<'_>,
) -> Option<InteractionResult> {
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

/// How a carried thing is drawn: a block as its item (a chest's or a furnace's own 3D model), a mob as its model
/// (its variant's texture), or, for a mob whose model or texture isn't found, its spawn egg.
#[derive(Clone)]
enum Shown {
    Item(ItemStack),
    Mob {
        parts: Vec<(Arc<ModelPart>, String, u32)>,
        egg: Option<ItemStack>,
    },
}

/// What each player in view carries, by their entity id, and this client's own player's id.
struct ClientView {
    local: Option<u32>,
    shown: HashMap<u32, (Carried, Shown)>,
    /// Baked model layers by name (`minecraft:cow#main`), None for a layer the game hasn't.
    layers: HashMap<String, Option<Arc<ModelPart>>>,
    /// The arms of each player in view who carries something, held forward: [`Arms`].
    arms: HashMap<u32, Arms>,
    /// Which of them crouch: their arms and load go down and forward with the body, as vanilla's crouch does.
    crouching: HashSet<u32>,
}

/// How a crouch moves the arms (`HumanoidModel.setupAnim`: from 2 to 5.2 texels down) and what they hold.
const CROUCH_ARMS_DOWN: f32 = 3.2;
const CROUCH_LOAD: [f32; 3] = [0.0, -0.2, 0.12];

/// A carrying player's arms, drawn forward in place of the model's own: the skin sprites to try, in order (the
/// downloaded skin, then the default one), and the arm parts of their model.
#[derive(Clone)]
struct Arms {
    sprites: Vec<String>,
    right: Arc<ModelPart>,
    left: Arc<ModelPart>,
}

/// The arms held forward: up from hanging by this much (radians) and a little in towards each other.
const ARMS_UP: f32 = -0.95;
const ARMS_IN: f32 = 0.15;

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let (mut out, mut bits, mut acc) = (Vec::new(), 0u32, 0u32);
    for c in text
        .bytes()
        .filter(|c| !c.is_ascii_whitespace() && *c != b'=')
    {
        acc = (acc << 6) | u32::from(value(c)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// The atlas sprite of a downloaded skin, as Horizon names it (`skins::skin_location`): `skins/` and the
/// SHA-1 of the UTF-16 code units of the texture's hash, the last part of its URL without an extension.
fn skin_sprite(textures_property: &str) -> Option<String> {
    let json: mod_api::serde_json::Value =
        mod_api::serde_json::from_slice(&base64_decode(textures_property)?).ok()?;
    let url = json.get("textures")?.get("SKIN")?.get("url")?.as_str()?;
    let path = url.split_once("://").map_or(url, |(_, rest)| rest);
    let path = &path[..path.find(['?', '#']).unwrap_or(path.len())];
    let name = &path[path.rfind('/').map_or(0, |i| i + 1)..];
    let hash = &name[..name.rfind('.').unwrap_or(name.len())];
    let units: Vec<u8> = hash.encode_utf16().flat_map(u16::to_le_bytes).collect();
    use sha1::Digest;
    let digest = sha1::Sha1::digest(&units);
    Some(
        digest
            .iter()
            .fold(String::from("skins/"), |out, b| out + &format!("{b:02x}")),
    )
}

/// `DefaultPlayerSkin.get(UUID)`, as Horizon's `tab_list::default_skin`.
fn default_skin(uuid: u128) -> String {
    const NAMES: [&str; 9] = [
        "alex", "ari", "efe", "kai", "makena", "noor", "steve", "sunny", "zuri",
    ];
    let hilo = ((uuid >> 64) as u64) ^ (uuid as u64);
    let hash = ((hilo >> 32) as i32) ^ (hilo as i32);
    let i = hash.rem_euclid(18) as usize;
    format!(
        "entity/player/{}/{}",
        if i < 9 { "slim" } else { "wide" },
        NAMES[i % 9]
    )
}

fn arms_of(
    host: &dyn ClientHost,
    layers: &mut HashMap<String, Option<Arc<ModelPart>>>,
    player: PlayerRef,
) -> Option<Arms> {
    let slim = host
        .player_skin(player.entity())
        .is_some_and(|skin| skin.slim);
    let model = layer(host, layers, if slim { "player_slim" } else { "player" })?;
    let part = |name: &str| {
        model
            .children()
            .find(|(n, _)| *n == name)
            .map(|(_, part)| part.clone())
    };
    let (mut right, mut left) = (part("right_arm")?, part("left_arm")?);
    right.x_rot = ARMS_UP;
    right.y_rot = -ARMS_IN;
    left.x_rot = ARMS_UP;
    left.y_rot = ARMS_IN;
    let mut sprites: Vec<String> = player
        .profile(host)
        .into_iter()
        .flat_map(|profile| profile.properties)
        .filter(|property| property.name == "textures")
        .filter_map(|property| skin_sprite(&property.value))
        .collect();
    sprites.push(default_skin(player.uuid()));
    Some(Arms {
        sprites,
        right: Arc::new(right),
        left: Arc::new(left),
    })
}

static VIEW: Mutex<Option<ClientView>> = Mutex::new(None);

fn view() -> std::sync::MutexGuard<'static, Option<ClientView>> {
    VIEW.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn item_of(host: &dyn Host, id: &str) -> Option<ItemStack> {
    Some(ItemStack::of(host.item_key(&Identifier::parse(id).ok()?)?))
}

/// The 16 dye colours (`DyeColor.getTextureDiffuseColor`) a sheep's wool is tinted with, by `Color`.
const DYES: [u32; 16] = [
    0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52, 0x9D9D97,
    0x169C9C, 0x8932B8, 0x3C44AA, 0x835432, 0x5E7C16, 0xB02E26, 0x1D1D21,
];

/// The model layers (without `#main`) and textures a mob of `name` (`cow`) is drawn with, from its saved data:
/// its variant, whether it is a baby, tamed or shorn, and its wool's colour.
fn int(data: &Compound, key: &str) -> Option<i32> {
    match data.get(&key.into()) {
        Some(Tag::Int(v)) => Some(*v),
        Some(Tag::Byte(v)) => Some(i32::from(*v)),
        _ => None,
    }
}

fn mob_look(name: &str, data: &Compound) -> Vec<(String, String, u32)> {
    let baby = matches!(data.get(&"Age".into()), Some(Tag::Int(age)) if *age < 0);
    let b = if baby { "_baby" } else { "" };
    let variant =
        read_string(data, "variant").map(|v| v.trim_start_matches("minecraft:").to_owned());
    let v = variant.as_deref();
    let white = 0xFFFFFF;
    match name {
        "cow" | "pig" | "chicken" => {
            let v = v.unwrap_or("temperate");
            // only the cow has a warm model of its own; cold ones all have one
            let layer = match (name, v) {
                ("cow", "warm") | (_, "cold") if !baby => format!("{v}_{name}"),
                ("cow", "warm") | ("cow", "cold") => format!("{v}_{name}{b}"),
                _ => format!("{name}{b}"),
            };
            vec![(layer, format!("entity/{name}/{name}_{v}{b}"), white)]
        }
        "wolf" => {
            let tame = if data.contains_key(&"Owner".into()) {
                "_tame"
            } else {
                ""
            };
            let texture = match v {
                None | Some("pale") => format!("entity/wolf/wolf{tame}{b}"),
                Some(v) => format!("entity/wolf/wolf_{v}{tame}{b}"),
            };
            vec![(format!("wolf{b}"), texture, white)]
        }
        "cat" => vec![(
            format!("cat{b}"),
            format!("entity/cat/cat_{}{b}", v.unwrap_or("tabby")),
            white,
        )],
        "sheep" => {
            let mut look = vec![(format!("sheep{b}"), format!("entity/sheep/sheep{b}"), white)];
            if !matches!(data.get(&"Sheared".into()), Some(Tag::Byte(1))) {
                let color = match data.get(&"Color".into()) {
                    Some(Tag::Byte(c)) => DYES.get(*c as usize).copied().unwrap_or(white),
                    _ => white,
                };
                look.push((
                    format!("sheep{b}#wool"),
                    format!("entity/sheep/sheep_wool{b}"),
                    color,
                ));
            }
            look
        }
        // fish share entity/fish/; their models go by size
        "cod" => vec![("cod".into(), "entity/fish/cod".into(), white)],
        "salmon" => {
            let layer = match read_string(data, "type").as_deref() {
                Some("small") => "salmon_small",
                Some("large") => "salmon_large",
                _ => "salmon",
            };
            vec![(layer.into(), "entity/fish/salmon".into(), white)]
        }
        "pufferfish" => {
            let layer = match data.get(&"PuffState".into()) {
                Some(Tag::Int(2)) => "pufferfish_big",
                Some(Tag::Int(1)) => "pufferfish_medium",
                _ => "pufferfish_small",
            };
            vec![(layer.into(), "entity/fish/pufferfish".into(), white)]
        }
        "tropical_fish" => {
            // `Variant`: the shape (0 small, 1 large), its pattern, the body's and the pattern's dye colours
            let packed = match data.get(&"Variant".into()) {
                Some(Tag::Int(v)) => *v as u32,
                _ => 0,
            };
            let (size, letter) = if packed & 0xFF == 1 {
                ("large", "b")
            } else {
                ("small", "a")
            };
            let dye = |shift: u32| {
                DYES.get(((packed >> shift) & 0xFF) as usize)
                    .copied()
                    .unwrap_or(white)
            };
            let pattern = ((packed >> 8) & 0xFF) % 6 + 1;
            vec![
                (
                    format!("tropical_fish_{size}"),
                    format!("entity/fish/tropical_{letter}"),
                    dye(16),
                ),
                (
                    format!("tropical_fish_{size}#pattern"),
                    format!("entity/fish/tropical_{letter}_pattern_{pattern}"),
                    dye(24),
                ),
            ]
        }
        "glow_squid" => vec![(
            format!("glow_squid{b}"),
            format!("entity/squid/glow_squid{b}"),
            white,
        )],
        "axolotl" => {
            const NAMES: [&str; 5] = ["lucy", "wild", "gold", "cyan", "blue"];
            let variant = match (v, data.get(&"Variant".into())) {
                (Some(v), _) => v.to_owned(),
                (None, Some(Tag::Int(i))) => {
                    NAMES.get(*i as usize).copied().unwrap_or("lucy").to_owned()
                }
                _ => "lucy".to_owned(),
            };
            vec![(
                format!("axolotl{b}"),
                format!("entity/axolotl/axolotl_{variant}{b}"),
                white,
            )]
        }
        "horse" => {
            const COLORS: [&str; 7] = [
                "white",
                "creamy",
                "chestnut",
                "brown",
                "black",
                "gray",
                "darkbrown",
            ];
            const MARKINGS: [&str; 5] = ["", "white", "whitefield", "whitedots", "blackdots"];
            let packed = int(data, "Variant").unwrap_or(0) as u32;
            let color = v.unwrap_or_else(|| {
                COLORS
                    .get((packed & 0xFF) as usize)
                    .copied()
                    .unwrap_or("white")
            });
            let mut look = vec![(
                format!("horse{b}"),
                format!("entity/horse/horse_{color}{b}"),
                white,
            )];
            if let Some(marks) = MARKINGS
                .get(((packed >> 8) & 0xFF) as usize)
                .filter(|m| !m.is_empty())
            {
                look.push((
                    format!("horse{b}"),
                    format!("entity/horse/horse_markings_{marks}{b}"),
                    white,
                ));
            }
            look
        }
        "donkey" | "mule" => vec![(
            format!("{name}{b}"),
            format!("entity/horse/{name}{b}"),
            white,
        )],
        "skeleton_horse" => vec![(
            format!("skeleton_horse{b}"),
            format!("entity/horse/horse_skeleton{b}"),
            white,
        )],
        "zombie_horse" => vec![(
            format!("zombie_horse{b}"),
            format!("entity/horse/horse_zombie{b}"),
            white,
        )],
        "llama" | "trader_llama" => {
            const COLORS: [&str; 4] = ["creamy", "white", "brown", "gray"];
            let color = v.unwrap_or_else(|| {
                COLORS
                    .get(int(data, "Variant").unwrap_or(0) as usize)
                    .copied()
                    .unwrap_or("creamy")
            });
            vec![(
                format!("{name}{b}"),
                format!("entity/llama/llama_{color}{b}"),
                white,
            )]
        }
        "mooshroom" => {
            let kind = read_string(data, "Type").unwrap_or_else(|| v.unwrap_or("red").to_owned());
            vec![(
                format!("mooshroom{b}"),
                format!("entity/cow/mooshroom_{kind}{b}"),
                white,
            )]
        }
        "ocelot" => vec![(format!("ocelot{b}"), format!("entity/cat/ocelot{b}"), white)],
        "parrot" => {
            const COLORS: [&str; 5] = ["red_blue", "blue", "green", "yellow_blue", "grey"];
            let color = v.unwrap_or_else(|| {
                COLORS
                    .get(int(data, "Variant").unwrap_or(0) as usize)
                    .copied()
                    .unwrap_or("red_blue")
            });
            vec![(
                "parrot".into(),
                format!("entity/parrot/parrot_{color}"),
                white,
            )]
        }
        "polar_bear" => vec![(
            format!("polar_bear{b}"),
            format!("entity/bear/polarbear{b}"),
            white,
        )],
        "rabbit" => {
            let kind = v.map(str::to_owned).unwrap_or_else(|| {
                match int(data, "RabbitType").unwrap_or(0) {
                    1 => "white",
                    2 => "black",
                    3 => "white_splotched",
                    4 => "gold",
                    5 => "salt",
                    99 => "caerbannog",
                    _ => "brown",
                }
                .to_owned()
            });
            vec![(
                format!("rabbit{b}"),
                format!("entity/rabbit/rabbit_{kind}{b}"),
                white,
            )]
        }
        "villager" => {
            // `VillagerData`: its biome's clothes, its profession's over them, a badge of its level
            let info = read_compound(data, "VillagerData").unwrap_or_default();
            let id = |key| {
                read_string(&info, key).map(|v| v.trim_start_matches("minecraft:").to_owned())
            };
            let kind = id("type").unwrap_or_else(|| "plains".into());
            let model = format!("villager{b}");
            let mut look = vec![(model.clone(), format!("entity/villager/villager{b}"), white)];
            if baby {
                look.push((model, format!("entity/villager/baby/{kind}"), white));
            } else {
                look.push((model.clone(), format!("entity/villager/type/{kind}"), white));
                if let Some(job) = id("profession").filter(|p| p != "none") {
                    look.push((
                        model.clone(),
                        format!("entity/villager/profession/{job}"),
                        white,
                    ));
                    const LEVELS: [&str; 5] = ["stone", "iron", "gold", "emerald", "diamond"];
                    let level = int(&info, "level").unwrap_or(1).clamp(1, 5) as usize;
                    if job != "nitwit" {
                        look.push((
                            model,
                            format!("entity/villager/profession_level/{}", LEVELS[level - 1]),
                            white,
                        ));
                    }
                }
            }
            look
        }
        "happy_ghast" => vec![(
            format!("happy_ghast{b}"),
            format!("entity/ghast/happy_ghast{b}"),
            white,
        )],
        "sniffer" if baby => vec![(
            "sniffer_baby".into(),
            "entity/sniffer/snifflet".into(),
            white,
        )],
        "frog" => vec![(
            "frog".into(),
            format!("entity/frog/frog_{}", v.unwrap_or("temperate")),
            white,
        )],
        _ => vec![(
            format!("{name}{b}"),
            format!("entity/{name}/{name}{b}"),
            white,
        )],
    }
}

fn layer(
    host: &dyn ClientHost,
    layers: &mut HashMap<String, Option<Arc<ModelPart>>>,
    name: &str,
) -> Option<Arc<ModelPart>> {
    let name = if name.contains('#') {
        format!("minecraft:{name}")
    } else {
        format!("minecraft:{name}#main")
    };
    layers
        .entry(name)
        .or_insert_with_key(|name| host.bake_layer(name).map(Arc::new))
        .clone()
}

fn shown(
    host: &dyn ClientHost,
    layers: &mut HashMap<String, Option<Arc<ModelPart>>>,
    what: &Carried,
) -> Option<Shown> {
    match what {
        Carried::Block { state, .. } => item_of(host, block_id(state)).map(Shown::Item),
        Carried::Entity { kind, data, .. } => {
            let name = kind.trim_start_matches("minecraft:");
            let parts: Option<Vec<_>> = mob_look(name, data)
                .into_iter()
                .map(|(model, texture, tint)| {
                    // a baby without a model of its own is drawn as the grown-up one
                    let part = layer(host, layers, &model)
                        .or_else(|| layer(host, layers, &model.replace("_baby", "")))?;
                    Some((part, texture, tint))
                })
                .collect();
            let egg = item_of(host, &format!("{kind}_spawn_egg"));
            match parts {
                Some(parts) if !parts.is_empty() => Some(Shown::Mob { parts, egg }),
                _ => egg.map(Shown::Item),
            }
        }
    }
}

/// A carried animal is alive: it looks about, its legs dangle, its tail wags and a bird flaps, by the names of
/// its model's parts (every layer of one mob moves alike, so a sheep's wool follows its body), from the
/// animation clock in ticks.
fn wiggle(part: &mut ModelPart, time: f32) {
    for (name, child) in part.children_mut() {
        let side = if name.starts_with("left") { 1.0 } else { -1.0 };
        let front = if name.contains("front") { 1.0 } else { -1.0 };
        match name {
            "head" => {
                child.y_rot += (time * 0.045).sin() * 0.55;
                child.x_rot += (time * 0.07).sin() * 0.2;
            }
            _ if name.contains("leg") || name.contains("haunch") => {
                child.x_rot += (time * 0.22 + side * front).sin() * 0.35
            }
            _ if name.contains("wing") => child.z_rot += side * (0.4 + (time * 0.9).sin() * 0.4),
            _ if name.starts_with("tail") => child.y_rot += (time * 0.3).sin() * 0.5,
            _ if name.ends_with("_ear") => child.x_rot += (time * 0.1 + side).sin() * 0.15,
            _ => {}
        }
        wiggle(child, time);
    }
}

/// Draws `shown` centred on the pose's origin, about `size` blocks across.
fn draw_shown(draw: &mut dyn BlockEntityDrawContext, shown: &Shown, size: f32) {
    match shown {
        Shown::Item(stack) => {
            // an item's `fixed` display is half a block
            draw.push();
            draw.scale([size * 2.0; 3]);
            draw.item(stack, ItemDisplayContext::Fixed, None, 0);
            draw.pop();
        }
        Shown::Mob { parts, egg } => {
            draw.push();
            // a mob model's own frame: +y down, its front towards -z, its feet 24 texels down; centred
            draw.scale([size, -size, -size]);
            draw.translate([0.0, -1.0, 0.0]);
            // and it shifts in the arms
            let time = draw.animation_time() as f32;
            draw.rotate_about([0.0, 0.0, 1.0], (time * 0.05).sin() * 4.0, [0.0, 1.0, 0.0]);
            let time = draw.animation_time() as f32;
            let mut drew = false;
            for (part, texture, tint) in parts {
                let mut part = (**part).clone();
                wiggle(&mut part, time);
                // a baby without a texture of its own wears the grown-up one
                drew |= draw.model_part(&part, texture, *tint)
                    || draw.model_part(&part, &texture.replace("_baby", ""), *tint);
            }
            draw.pop();
            if !drew && let Some(egg) = egg {
                draw_shown(draw, &Shown::Item(egg.clone()), size);
            }
        }
    }
}

/// Draws what a player in third person carries in front of their chest.
struct CarriedLayer;

impl EntityRenderLayer for CarriedLayer {
    fn submit(
        &self,
        entity: &EntityRenderState,
        _parts: Option<&PosedParts>,
        draw: &mut dyn BlockEntityDrawContext,
    ) -> Result<(), String> {
        let (shown, crouching) = {
            let view = view();
            let view = view.as_ref();
            (
                view.and_then(|view| view.shown.get(&entity.id).map(|(_, shown)| shown.clone())),
                view.is_some_and(|view| view.crouching.contains(&entity.id)),
            )
        };
        if let Some(shown) = shown {
            draw.push();
            // at the feet, +z the way the player faces
            draw.translate([0.0, 1.0, 0.55]);
            if crouching {
                draw.translate(CROUCH_LOAD);
            }
            draw_shown(draw, &shown, 0.7);
            draw.pop();
        }
        Ok(())
    }
}

/// Players slowed down by this mod, so the modifier is added and taken away once.
static SLOWED: Mutex<Option<HashSet<u128>>> = Mutex::new(None);

fn slowdown_id() -> Option<Identifier> {
    Identifier::parse(&format!("{NAMESPACE}:carrying")).ok()
}

fn update_slowdown(host: &mut dyn ServerHost, player: PlayerRef) {
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
        let modifier = AttributeModifier::new(
            id,
            -config::rules().slowdown,
            AttributeOperation::AddMultipliedTotal,
        );
        living.add_attribute_modifier(
            host,
            "minecraft:movement_speed",
            modifier,
            ModifierLifetime::Transient,
        );
        slowed.insert(uuid);
    } else {
        living.remove_attribute_modifier(host, "minecraft:movement_speed", &id);
        slowed.remove(&uuid);
    }
}

/// Takes this mod's server config; a file that doesn't parse is logged and the rules before it stay.
fn take_config(host: HostSide, file: &ConfigFile) {
    if file.mod_id != NAMESPACE {
        return;
    }
    if let Err(why) = config::load(&file.text)
        && let HostSide::Server(host) = host
    {
        let line = format!("{}: {why}; the rules before it stay", file.file);
        let _ = host
            .messages()
            .console_log(NAMESPACE, ConsoleLevel::Warn, &line);
    }
}

#[derive(Default)]
pub struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let click = event.click;
        if click.hand != Hand::Main {
            return Flow::Continue;
        }
        let answer = match host {
            HostSide::Server(host) => server_click(host, click),
            HostSide::Client(host) => client_carrying(host).then_some(InteractionResult::Success),
        };
        match answer {
            Some(result) => {
                *event.cancellation_result = result;
                event.cancelled = true;
                Flow::Cancel
            }
            None => Flow::Continue,
        }
    }

    fn entity_interact(
        &self,
        host: HostSide,
        event: &mut EntityInteractEvent<'_>,
    ) -> Option<InteractionResult> {
        if event.hand != Hand::Main {
            return None;
        }
        match host {
            HostSide::Server(host) => server_pick_mob(host, event),
            HostSide::Client(host) => client_carrying(host).then_some(InteractionResult::Fail),
        }
    }

    fn register_configs(&self, _host: HostSide, event: &mut RegisterConfigsEvent<'_>) {
        event
            .configs
            .push(Config::new(NAMESPACE, ConfigKind::Server, config::DEFAULT));
    }

    fn config_loading(&self, host: HostSide, event: &mut ConfigLoadingEvent<'_>) {
        take_config(host, event.config);
    }

    fn config_reloading(&self, host: HostSide, event: &mut ConfigReloadingEvent<'_>) {
        take_config(host, event.config);
    }

    /// Hands full: no item is used while carrying (the click puts the load down instead, on a block).
    fn right_click_item(&self, host: HostSide, event: &mut RightClickItemEvent<'_>) -> Flow {
        let carrying = match host {
            HostSide::Server(host) => carried(host, event.click.player).is_some(),
            HostSide::Client(host) => client_carrying(host),
        };
        if !carrying {
            return Flow::Continue;
        }
        *event.cancellation_result = InteractionResult::Fail;
        event.cancelled = true;
        Flow::Cancel
    }

    /// Hands full: no hitting.
    fn attack_entity(&self, host: &mut dyn ServerHost, event: &mut AttackEntityEvent<'_>) -> Flow {
        if !acting_carries(host) {
            return Flow::Continue;
        }
        event.cancelled = true;
        Flow::Cancel
    }

    /// Hands full: no digging.
    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        if !acting_carries(host) {
            return Flow::Continue;
        }
        event.cancelled = true;
        Flow::Cancel
    }

    /// Hands full: the hotbar stays on its slot, so nothing else comes into the hand.
    fn player_switch_hotbar_slot_pre(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerSwitchHotbarSlotPreEvent,
    ) -> Flow {
        if carried(host, event.player).is_none() {
            return Flow::Continue;
        }
        event.cancelled = true;
        Flow::Cancel
    }

    fn player_tick_post(&self, host: HostSide, event: &mut PlayerTickPostEvent) {
        if let HostSide::Server(host) = host {
            update_slowdown(host, event.player);
        }
    }

    fn player_logged_out(&self, _host: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        // the modifier is transient: a player who comes back is slowed again by the next tick
        if let Some(slowed) = SLOWED
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .as_mut()
        {
            slowed.remove(&event.player);
        }
    }

    /// A player who dies puts down what they carry where they died (or a little higher, when that is taken).
    fn living_death(&self, host: &mut dyn ServerHost, event: &mut LivingDeathEvent<'_>) -> Flow {
        if event.kind != "minecraft:player" {
            return Flow::Continue;
        }
        let Some(entity) = host.entity(event.entity) else {
            return Flow::Continue;
        };
        let Some(uuid) = entity.uuid(host) else {
            return Flow::Continue;
        };
        let Some(what) = carried(host, uuid) else {
            return Flow::Continue;
        };
        let (Some(player), Some(level), Some([x, y, z])) = (
            host.player(uuid),
            entity.level(host),
            entity.block_position(host),
        ) else {
            return Flow::Continue;
        };
        let dimension = dimension_of(host, player).unwrap_or_default();
        let free = |host: &dyn Host, pos| {
            level
                .block_state(host, pos)
                .and_then(|state| host.block_states().replaceable(state))
                == Some(true)
        };
        // the first spot above where they died that takes it (a mob needs room for its whole body)
        let placed = (0..8)
            .map(|up| [x, y + up, z])
            .any(|pos| free(host, pos) && put_down(host, level, &dimension, pos, &what, None));
        if placed {
            let _ = CARRIED.set(host, Holder::Player(uuid), None);
        }
        Flow::Continue
    }

    /// Reads what this client's player and every listed player in view carry (synced attachments), so the
    /// render hooks, which get no host to read them with, find it by entity id.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        let local = host.local_player();
        let mut players: Vec<u128> = host
            .client_session()
            .client_tab_list()
            .map(|list| list.entries.iter().map(|e| e.uuid).collect())
            .unwrap_or_default();
        players.extend(local.map(PlayerRef::uuid));
        let mut carried_by = Vec::new();
        for uuid in players {
            let Some(player) = host.player(uuid) else {
                continue;
            };
            if let Some(what) = CARRIED
                .get_existing(host, Holder::Player(uuid))
                .ok()
                .flatten()
                .flatten()
            {
                carried_by.push((player, what));
            }
        }
        let mut view = view();
        let view = view.get_or_insert_with(|| ClientView {
            local: None,
            shown: HashMap::new(),
            layers: HashMap::new(),
            arms: HashMap::new(),
            crouching: HashSet::new(),
        });
        view.local = local.map(|player| player.entity().id());
        let mut shown = HashMap::new();
        let mut arms = HashMap::new();
        view.crouching.clear();
        for (player, what) in carried_by {
            let id = player.entity().id();
            if player.entity().is_crouching(host) == Some(true) {
                view.crouching.insert(id);
            }
            if let Some(held) = view
                .arms
                .remove(&id)
                .or_else(|| arms_of(host, &mut view.layers, player))
            {
                arms.insert(id, held);
            }
            let known = view.shown.remove(&id).filter(|(was, _)| *was == what);
            let entry = match known {
                Some(entry) => Some(entry),
                None => self::shown(host, &mut view.layers, &what).map(|look| (what, look)),
            };
            if let Some(entry) = entry {
                shown.insert(id, entry);
            }
        }
        view.shown = shown;
        view.arms = arms;
    }

    /// A player who carries something holds their arms forward: the model's arms are drawn here, posed, with
    /// the player's skin, and the model's own left out; when no skin sprite draws, the model keeps its arms.
    fn render_player_pre(
        &self,
        _host: &dyn ClientHost,
        event: &mut RenderPlayerPreEvent<'_>,
    ) -> Flow {
        let (arms, crouching) = {
            let view = view();
            let view = view.as_ref();
            (
                view.and_then(|view| view.arms.get(&event.player.id).cloned()),
                view.is_some_and(|view| view.crouching.contains(&event.player.id)),
            )
        };
        let Some(mut arms) = arms else {
            return Flow::Continue;
        };
        if crouching {
            for part in [&mut arms.right, &mut arms.left] {
                Arc::make_mut(part).y += CROUCH_ARMS_DOWN;
            }
        }
        let draw = &mut *event.draw;
        draw.push();
        // the model's own frame, as `LivingEntityRenderer` and a player's 0.9375 scale give it
        draw.scale([0.9375, -0.9375, -0.9375]);
        draw.translate([0.0, -1.501, 0.0]);
        let drew = arms.sprites.iter().any(|sprite| {
            draw.model_part(&arms.right, sprite, 0xFFFFFF)
                && draw.model_part(&arms.left, sprite, 0xFFFFFF)
        });
        draw.pop();
        if drew {
            event.hidden.hide(HumanoidPart::RightArm);
            event.hidden.hide(HumanoidPart::LeftArm);
        }
        Flow::Continue
    }

    fn register_entity_layers(
        &self,
        _context: &RegistrationContext,
        event: &mut RegisterEntityLayersEvent<'_>,
    ) {
        for model in event.layers.player_models() {
            let _ = event
                .layers
                .add(LayerTarget::Player(model), Arc::new(CarriedLayer));
        }
    }

    /// The carried thing is held in front with both hands: drawn once, below the view and tilted so its top
    /// shows, instead of the arms.
    fn render_hand(&self, host: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
        let (shown, arms) = {
            let view = view();
            let view = view.as_ref();
            let local = view.and_then(|view| view.local);
            let get = |id| {
                view.and_then(|view| {
                    Some((view.shown.get(&id)?.1.clone(), view.arms.get(&id).cloned()))
                })
            };
            match local.and_then(get) {
                Some(found) => found,
                None => return Flow::Continue,
            }
        };
        if event.render.hand == Hand::Main {
            let draw = &mut *event.draw;
            let low = -0.62 - event.render.equip * 0.4;
            // both arms reach forward along the sides of what is carried, in the player's own skin
            if let (Some(arms), Some((sprite, _))) = (arms, host.local_player_arm_skin()) {
                for (part, side) in [(&arms.right, 1.0f32), (&arms.left, -1.0)] {
                    let mut part = (**part).clone();
                    (part.x, part.y, part.z, part.x_rot, part.y_rot, part.z_rot) =
                        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
                    draw.push();
                    draw.translate([0.42 * side, low - 0.05, -0.3]);
                    // the arm's length (+y, down in the model) points into the screen, a little inwards
                    draw.rotate_about([0.0, 1.0, 0.0], 8.0 * side, [0.0; 3]);
                    draw.rotate_about([1.0, 0.0, 0.0], -90.0, [0.0; 3]);
                    draw.model_part(&part, sprite, 0xFFFFFF);
                    draw.pop();
                }
            }
            draw.push();
            draw.translate([0.0, low, -1.0]);
            draw.rotate_about([1.0, 0.0, 0.0], 20.0, [0.0; 3]);
            if matches!(shown, Shown::Mob { .. }) {
                // a mob looks the way the player does
                draw.rotate_about([0.0, 1.0, 0.0], 180.0, [0.0; 3]);
            }
            draw_shown(draw, &shown, 0.75);
            draw.pop();
        }
        event.cancelled = true;
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_are_read_and_changed_as_text() {
        let chest = "minecraft:chest[facing=north,type=left,waterlogged=false]";
        assert_eq!(block_id(chest), "minecraft:chest");
        assert_eq!(property(chest, "type"), Some("left"));
        assert_eq!(property(chest, "nothing"), None);
        assert_eq!(
            placed_state(chest, Some("east")),
            "minecraft:chest[facing=west,type=single,waterlogged=false]"
        );
        // a barrel facing up stays up; a block without properties stays as it is
        assert_eq!(
            placed_state("minecraft:barrel[facing=up,open=false]", Some("north")),
            "minecraft:barrel[facing=up,open=false]"
        );
        assert_eq!(
            placed_state("minecraft:beacon", Some("north")),
            "minecraft:beacon"
        );
    }

    #[test]
    fn the_attachment_round_trips() {
        let mut data = Compound::new();
        data.insert("Items".into(), Tag::List(Vec::new()));
        for value in [
            None,
            Some(Carried::Block {
                state: "minecraft:furnace[facing=south,lit=false]".into(),
                data: Some(data.clone()),
            }),
            Some(Carried::Block {
                state: "minecraft:bell".into(),
                data: None,
            }),
            Some(Carried::Entity {
                kind: "minecraft:cow".into(),
                data,
                size: [0.9, 1.4],
            }),
        ] {
            assert_eq!(decode(&encode(&value).unwrap()).unwrap(), value);
        }
    }

    #[test]
    fn mobs_are_drawn_with_their_variant() {
        let data = |pairs: &[(&str, Tag)]| -> Compound {
            pairs
                .iter()
                .map(|(k, v)| ((*k).into(), v.clone()))
                .collect()
        };
        let look = |name, d: &Compound| {
            mob_look(name, d)
                .into_iter()
                .map(|(l, t, _)| (l, t))
                .collect::<Vec<_>>()
        };
        let pair = |l: &str, t: &str| (l.to_owned(), t.to_owned());
        assert_eq!(
            look("cow", &data(&[])),
            [pair("cow", "entity/cow/cow_temperate")]
        );
        assert_eq!(
            look("cow", &data(&[("variant", string("minecraft:cold"))])),
            [pair("cold_cow", "entity/cow/cow_cold")]
        );
        assert_eq!(
            look(
                "cow",
                &data(&[
                    ("variant", string("minecraft:warm")),
                    ("Age", Tag::Int(-2000))
                ])
            ),
            [pair("warm_cow_baby", "entity/cow/cow_warm_baby")]
        );
        assert_eq!(
            look("pig", &data(&[("variant", string("minecraft:warm"))])),
            [pair("pig", "entity/pig/pig_warm")]
        );
        assert_eq!(
            look("pig", &data(&[("variant", string("minecraft:cold"))])),
            [pair("cold_pig", "entity/pig/pig_cold")]
        );
        assert_eq!(look("wolf", &data(&[])), [pair("wolf", "entity/wolf/wolf")]);
        assert_eq!(
            look(
                "wolf",
                &data(&[
                    ("variant", string("minecraft:ashen")),
                    ("Owner", Tag::IntArray(vec![1, 2, 3, 4]))
                ])
            ),
            [pair("wolf", "entity/wolf/wolf_ashen_tame")]
        );
        assert_eq!(
            look("sheep", &data(&[("Sheared", Tag::Byte(1))])),
            [pair("sheep", "entity/sheep/sheep")]
        );
        let sheep = mob_look("sheep", &data(&[("Color", Tag::Byte(14))]));
        assert_eq!(
            sheep[1],
            (
                "sheep#wool".to_owned(),
                "entity/sheep/sheep_wool".to_owned(),
                0xB02E26
            )
        );
        assert_eq!(look("goat", &data(&[])), [pair("goat", "entity/goat/goat")]);
        let job: Compound = [
            ("type".into(), string("minecraft:snow")),
            ("profession".into(), string("minecraft:farmer")),
            ("level".into(), Tag::Int(3)),
        ]
        .into();
        assert_eq!(
            look("villager", &data(&[("VillagerData", Tag::Compound(job))])),
            [
                pair("villager", "entity/villager/villager"),
                pair("villager", "entity/villager/type/snow"),
                pair("villager", "entity/villager/profession/farmer"),
                pair("villager", "entity/villager/profession_level/gold"),
            ]
        );
        assert_eq!(look("cod", &data(&[])), [pair("cod", "entity/fish/cod")]);
        assert_eq!(
            look("salmon", &data(&[("type", string("large"))])),
            [pair("salmon_large", "entity/fish/salmon")]
        );
        assert_eq!(
            look("pufferfish", &data(&[("PuffState", Tag::Int(2))])),
            [pair("pufferfish_big", "entity/fish/pufferfish")]
        );
        // a large fish with pattern 2, an orange (1) body and a white (0) pattern
        let fish = mob_look(
            "tropical_fish",
            &data(&[("Variant", Tag::Int(1 | (1 << 8) | (1 << 16)))]),
        );
        assert_eq!(
            fish[0],
            (
                "tropical_fish_large".to_owned(),
                "entity/fish/tropical_b".to_owned(),
                DYES[1]
            )
        );
        assert_eq!(
            fish[1],
            (
                "tropical_fish_large#pattern".to_owned(),
                "entity/fish/tropical_b_pattern_2".to_owned(),
                DYES[0]
            )
        );
        assert_eq!(
            look("axolotl", &data(&[("Variant", Tag::Int(3))])),
            [pair("axolotl", "entity/axolotl/axolotl_cyan")]
        );
    }

    /// Every living type that isn't a monster finds its model and texture in the game's resources (adult and baby):
    /// `HORIZON_RESOURCES=<horizon-prepare output> cargo test every_mob -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn every_mob_has_a_look() {
        use mod_api::serde_json::Value;
        let root = std::path::PathBuf::from(
            std::env::var("HORIZON_RESOURCES").expect("HORIZON_RESOURCES"),
        );
        let read = |name: &str| -> Value {
            mod_api::serde_json::from_slice(&std::fs::read(root.join(name)).unwrap()).unwrap()
        };
        let (types, models) = (read("entity-types.json"), read("entity-models.json"));
        let mut missing = Vec::new();
        for (kind, info) in types.as_object().unwrap() {
            let living = info
                .get("attributes")
                .is_some_and(|a| a.as_object().is_some_and(|a| !a.is_empty()));
            if !living
                || info["category"] == "monster"
                || [
                    "minecraft:player",
                    "minecraft:armor_stand",
                    "minecraft:mannequin",
                ]
                .contains(&kind.as_str())
            {
                continue;
            }
            let name = kind.trim_start_matches("minecraft:");
            for age in [0, -24000] {
                let data: Compound = [("Age".into(), Tag::Int(age))].into();
                for (layer, texture, _) in mob_look(name, &data) {
                    let layer = if layer.contains('#') {
                        format!("minecraft:{layer}")
                    } else {
                        format!("minecraft:{layer}#main")
                    };
                    // as `shown` does: a baby without a model of its own is drawn as the grown-up one
                    let grown = layer.replace("_baby", "");
                    if models.get(&layer).is_none() && models.get(&grown).is_none() {
                        missing.push(format!("{kind}: model {layer}"));
                    }
                    let file = |t: &str| {
                        root.join(format!("assets/minecraft/textures/{t}.png"))
                            .exists()
                    };
                    if !file(&texture) && !file(&texture.replace("_baby", "")) {
                        missing.push(format!("{kind}: texture {texture}"));
                    }
                }
            }
        }
        missing.dedup();
        assert!(
            missing.is_empty(),
            "{} missing:\n{}",
            missing.len(),
            missing.join("\n")
        );
    }

    #[test]
    fn skins_are_named_as_horizon_names_them() {
        let textures = "eyJ0ZXh0dXJlcyI6IHsiU0tJTiI6IHsidXJsIjogImh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvYWJjMTIzIn19fQ==";
        assert_eq!(
            skin_sprite(textures).as_deref(),
            Some("skins/689307d2fc53af0fb941bc1bb42737ce4f3ef540")
        );
        assert_eq!(skin_sprite("bm90IGpzb24="), None);
        assert_eq!(
            default_skin(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210),
            "entity/player/slim/alex"
        );
    }

    #[test]
    fn owner_uuids_are_read_from_four_ints() {
        let uuid = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210u128;
        let ints = [
            0x0123_4567,
            0x89ab_cdefu32 as i32,
            0xfedc_ba98u32 as i32,
            0x7654_3210,
        ];
        assert_eq!(uuid_from_ints(&ints), Some(uuid));
        assert_eq!(uuid_from_ints(&ints[..3]), None);
    }
}
