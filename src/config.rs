//! The rules: `craftorio_carry-server.toml`, for each world (`serverconfig/`, else `config/`), on a dedicated
//! server and in single player (its built-in server). The game reads the file again when it changes. A file
//! that does not parse keeps the rules from before (the defaults at first).

use std::sync::RwLock;

/// The file written when there is none: every rule with its default and what it means.
pub const DEFAULT: &str = r#"# Craftorio Carry: sneak with both hands empty and right-click a block or a mob to pick it up, right-click a
# block to put it down. Ids may hold one * (any text): "minecraft:*_bed", "*_shulker_box".

[blocks]
# true: only blocks that hold something (a chest, a furnace, a barrel...); false: any block that can be broken
only_with_contents = true
# picked up even when only_with_contents would not let them
allow = []
# never picked up
deny = [
  "*_bed",
  "minecraft:end_portal",
  "minecraft:end_gateway",
  "minecraft:spawner",
  "minecraft:trial_spawner",
  "minecraft:vault",
  "minecraft:command_block",
  "minecraft:chain_command_block",
  "minecraft:repeating_command_block",
  "minecraft:structure_block",
  "minecraft:jigsaw",
  "minecraft:moving_piston",
  "minecraft:test_block",
  "minecraft:test_instance_block",
  "*_door",
  "minecraft:piston_head",
]

[mobs]
# whether monsters (zombies, creepers...) are picked up too
monsters = false
# whether a player picks up someone else's tamed pet
other_players_pets = false
# picked up even when monsters = false would not let them
allow = []
# never picked up (players never are)
deny = ["minecraft:ender_dragon", "minecraft:wither", "minecraft:warden", "minecraft:elder_guardian"]

[carrying]
# how much slower a carrying player walks: 0.25 is a quarter slower, 0 not at all
slowdown = 0.25
"#;

#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub only_with_contents: bool,
    pub allow_blocks: Vec<String>,
    pub deny_blocks: Vec<String>,
    pub monsters: bool,
    pub other_players_pets: bool,
    pub allow_mobs: Vec<String>,
    pub deny_mobs: Vec<String>,
    pub slowdown: f64,
}

impl Rules {
    /// The rules of `text`, a TOML file as [`DEFAULT`]: a key it lacks keeps its default.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let defaults: toml::Table = DEFAULT.parse().expect("the default config parses");
        let get = |section: &str, key: &str| {
            file.get(section)
                .and_then(|s| s.get(key))
                .or_else(|| defaults.get(section).and_then(|s| s.get(key)))
                .cloned()
                .ok_or_else(|| format!("[{section}] {key} is missing"))
        };
        let flag = |section: &str, key: &str| {
            get(section, key)?
                .as_bool()
                .ok_or_else(|| format!("[{section}] {key} must be true or false"))
        };
        let ids = |section: &str, key: &str| -> Result<Vec<String>, String> {
            let value = get(section, key)?;
            let list = value
                .as_array()
                .ok_or_else(|| format!("[{section}] {key} must be a list of ids"))?;
            list.iter()
                .map(|id| {
                    id.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("[{section}] {key}: {id} is not text"))
                })
                .collect()
        };
        let slowdown = get("carrying", "slowdown")?;
        let slowdown = slowdown
            .as_float()
            .or_else(|| slowdown.as_integer().map(|n| n as f64))
            .filter(|s| (0.0..=1.0).contains(s))
            .ok_or("[carrying] slowdown must be a number from 0 to 1")?;
        Ok(Rules {
            only_with_contents: flag("blocks", "only_with_contents")?,
            allow_blocks: ids("blocks", "allow")?,
            deny_blocks: ids("blocks", "deny")?,
            monsters: flag("mobs", "monsters")?,
            other_players_pets: flag("mobs", "other_players_pets")?,
            allow_mobs: ids("mobs", "allow")?,
            deny_mobs: ids("mobs", "deny")?,
            slowdown,
        })
    }

    /// Whether a block `id` is picked up; `has_contents`: it has a block entity.
    pub fn block(&self, id: &str, has_contents: bool) -> bool {
        !listed(&self.deny_blocks, id)
            && (has_contents || !self.only_with_contents || listed(&self.allow_blocks, id))
    }

    /// Whether a mob `kind` is picked up; `monster`: its type is an enemy.
    pub fn mob(&self, kind: &str, monster: bool) -> bool {
        kind != "minecraft:player"
            && !listed(&self.deny_mobs, kind)
            && (!monster || self.monsters || listed(&self.allow_mobs, kind))
    }
}

/// Whether `id` matches one of `patterns`: equal, or around a single `*`.
pub fn listed(patterns: &[String], id: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| match pattern.split_once('*') {
            None => pattern == id,
            Some((start, end)) => {
                id.len() >= start.len() + end.len() && id.starts_with(start) && id.ends_with(end)
            }
        })
}

static RULES: RwLock<Option<Rules>> = RwLock::new(None);

/// The rules in force.
pub fn rules() -> Rules {
    if let Some(rules) = RULES
        .read()
        .unwrap_or_else(|poison| poison.into_inner())
        .as_ref()
    {
        return rules.clone();
    }
    Rules::parse(DEFAULT).expect("the default config parses")
}

/// Takes the rules of `text`; on an error keeps the ones in force and says why.
pub fn load(text: &str) -> Result<(), String> {
    let rules = Rules::parse(text)?;
    *RULES.write().unwrap_or_else(|poison| poison.into_inner()) = Some(rules);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_carry_on_s() {
        let rules = Rules::parse(DEFAULT).unwrap();
        assert!(rules.block("minecraft:chest", true));
        assert!(!rules.block("minecraft:stone", false));
        assert!(!rules.block("minecraft:red_bed", true));
        assert!(!rules.block("minecraft:spawner", true));
        assert!(rules.mob("minecraft:cow", false));
        assert!(!rules.mob("minecraft:zombie", true));
        assert!(!rules.mob("minecraft:wither", false));
        assert!(!rules.mob("minecraft:player", false));
        assert_eq!(rules.slowdown, 0.25);
    }

    #[test]
    fn a_file_changes_only_what_it_names() {
        let rules = Rules::parse(
            "[blocks]\nonly_with_contents = false\ndeny = [\"*_shulker_box\"]\n[mobs]\nallow = [\"minecraft:creeper\"]\n[carrying]\nslowdown = 0\n",
        )
        .unwrap();
        assert!(rules.block("minecraft:stone", false));
        assert!(!rules.block("minecraft:red_shulker_box", true));
        // the deny list given replaces the default one
        assert!(rules.block("minecraft:red_bed", true));
        assert!(rules.mob("minecraft:creeper", true));
        assert!(!rules.mob("minecraft:zombie", true));
        assert_eq!(rules.slowdown, 0.0);
        assert!(!rules.other_players_pets);
    }

    #[test]
    fn a_bad_file_is_refused_with_the_reason() {
        assert!(Rules::parse("[blocks\n").is_err());
        assert_eq!(
            Rules::parse("[carrying]\nslowdown = 2\n").unwrap_err(),
            "[carrying] slowdown must be a number from 0 to 1"
        );
        assert_eq!(
            Rules::parse("[mobs]\nmonsters = 1\n").unwrap_err(),
            "[mobs] monsters must be true or false"
        );
    }

    #[test]
    fn patterns_take_one_star() {
        let list = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(listed(&list(&["minecraft:*_bed"]), "minecraft:red_bed"));
        assert!(!listed(&list(&["minecraft:*_bed"]), "minecraft:bedrock"));
        assert!(listed(&list(&["*"]), "anything"));
        assert!(!listed(&list(&["a*a"]), "a"));
    }
}
