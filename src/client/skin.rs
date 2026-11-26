//! The arms of a carrying player, drawn forward with their skin: the skin's sprite in the atlas, as Horizon
//! names it, and the arm parts of the player model.

use crate::prelude::*;

/// A carrying player's arms, drawn forward in place of the model's own: the skin sprites to try, in order (the
/// downloaded skin, then the default one), and the arm parts of their model.
#[derive(Clone)]
pub(crate) struct Arms {
    pub(crate) sprites: Vec<String>,
    pub(crate) right: Arc<ModelPart>,
    pub(crate) left: Arc<ModelPart>,
}

/// The arms held forward: up from hanging by this much (radians) and a little in towards each other.
pub(crate) const ARMS_UP: f32 = -0.95;
pub(crate) const ARMS_IN: f32 = 0.15;

pub(crate) fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    };
    let (mut out, mut bits, mut acc) = (Vec::new(), 0u32, 0u32);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
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
pub(crate) fn skin_sprite(textures_property: &str) -> Option<String> {
    let json: mod_api::serde_json::Value = mod_api::serde_json::from_slice(&base64_decode(textures_property)?).ok()?;
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
pub(crate) fn default_skin(uuid: u128) -> String {
    const NAMES: [&str; 9] = ["alex", "ari", "efe", "kai", "makena", "noor", "steve", "sunny", "zuri"];
    let hilo = ((uuid >> 64) as u64) ^ (uuid as u64);
    let hash = ((hilo >> 32) as i32) ^ (hilo as i32);
    let i = hash.rem_euclid(18) as usize;
    format!("entity/player/{}/{}", if i < 9 { "slim" } else { "wide" }, NAMES[i % 9])
}

pub(crate) fn arms_of(
    host: &dyn ClientHost,
    layers: &mut HashMap<String, Option<Arc<ModelPart>>>,
    player: PlayerRef,
) -> Option<Arms> {
    let slim = host.player_skin(player.entity()).is_some_and(|skin| skin.slim);
    let model = layer(host, layers, if slim { "player_slim" } else { "player" })?;
    let part = |name: &str| model.children().find(|(n, _)| *n == name).map(|(_, part)| part.clone());
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
