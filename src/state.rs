//! Block states as text (`minecraft:chest[facing=north,type=left]`), and how a carried block is put down.

/// `minecraft:chest[facing=north,type=left]` → `minecraft:chest`.
pub(crate) fn block_id(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

/// The value of `property` in a state string.
pub(crate) fn property<'a>(state: &'a str, property: &str) -> Option<&'a str> {
    let inner = state.split_once('[')?.1.strip_suffix(']')?;
    inner.split(',').find_map(|pair| {
        pair.split_once('=')
            .filter(|(key, _)| *key == property)
            .map(|(_, value)| value)
    })
}

/// The state string with `property` set to `value` (unchanged when the state has no such property).
pub(crate) fn with_property(state: &str, property: &str, value: &str) -> String {
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

pub(crate) fn opposite(direction: &str) -> Option<&'static str> {
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
pub(crate) fn placed_state(state: &str, player_facing: Option<&str>) -> String {
    let mut state = if property(state, "type").is_some_and(|t| t == "left" || t == "right") {
        with_property(state, "type", "single")
    } else {
        state.to_owned()
    };
    if let (Some(current), Some(facing)) = (property(&state, "facing"), player_facing.and_then(opposite))
        && opposite(current).is_some()
    {
        state = with_property(&state, "facing", facing);
    }
    state
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
        assert_eq!(placed_state("minecraft:beacon", Some("north")), "minecraft:beacon");
    }
}
