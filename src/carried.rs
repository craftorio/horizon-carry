//! What a player carries: a block (its state and contents) or a mob (its saved data and size), kept on the
//! player as an attachment, saved with them and synced to their client.

use crate::prelude::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Carried {
    /// A block state as `minecraft:chest[facing=north,...]` and its block entity's data.
    Block { state: String, data: Option<Compound> },
    /// An entity type and its saved data (without its UUID and position).
    /// `size`: its width and height in blocks, to find room for it again.
    Entity {
        kind: String,
        data: Compound,
        size: [f64; 2],
    },
}

pub(crate) static CARRIED: AttachmentType<Option<Carried>> = AttachmentType {
    namespace: NAMESPACE,
    name: "carried",
    default: || None,
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: true,
};

pub(crate) fn string(text: &str) -> Tag {
    Tag::String(text.into())
}

pub(crate) fn read_string(data: &Compound, key: &str) -> Option<String> {
    match data.get(&key.into()) {
        Some(Tag::String(text)) => Some(text.to_string_lossy()),
        _ => None,
    }
}

pub(crate) fn read_compound(data: &Compound, key: &str) -> Option<Compound> {
    match data.get(&key.into()) {
        Some(Tag::Compound(inner)) => Some(inner.clone()),
        _ => None,
    }
}

/// The model layers (without `#main`) and textures a mob of `name` (`cow`) is drawn with, from its saved data:
/// its variant, whether it is a baby, tamed or shorn, and its wool's colour.
pub(crate) fn int(data: &Compound, key: &str) -> Option<i32> {
    match data.get(&key.into()) {
        Some(Tag::Int(v)) => Some(*v),
        Some(Tag::Byte(v)) => Some(i32::from(*v)),
        _ => None,
    }
}

pub(crate) fn encode(value: &Option<Carried>) -> Result<Compound, AttachmentError> {
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

pub(crate) fn decode(data: &Compound) -> Result<Option<Carried>, AttachmentError> {
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

pub(crate) fn carried(host: &mut dyn ServerHost, player: u128) -> Option<Carried> {
    CARRIED
        .get_existing(host, Holder::Player(player))
        .ok()
        .flatten()
        .flatten()
}

pub(crate) fn uuid_from_ints(ints: &[i32]) -> Option<u128> {
    let [a, b, c, d] = ints else { return None };
    Some(
        [*a, *b, *c, *d]
            .iter()
            .fold(0u128, |uuid, part| (uuid << 32) | u128::from(*part as u32)),
    )
}

/// The size of a mob carried before sizes were kept: a cow's.
pub(crate) const DEFAULT_SIZE: [f64; 2] = [0.9, 1.4];

#[cfg(test)]
mod tests {
    use super::*;

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
    fn owner_uuids_are_read_from_four_ints() {
        let uuid = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210u128;
        let ints = [0x0123_4567, 0x89ab_cdefu32 as i32, 0xfedc_ba98u32 as i32, 0x7654_3210];
        assert_eq!(uuid_from_ints(&ints), Some(uuid));
        assert_eq!(uuid_from_ints(&ints[..3]), None);
    }
}
