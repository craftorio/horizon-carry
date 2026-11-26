//! The look rules: examples of each kind, and every living type that is not a monster against the game's
//! resources.

use super::*;

fn string(text: &str) -> Tag {
    crate::carried::string(text)
}

#[test]
fn mobs_are_drawn_with_their_variant() {
    let data = |pairs: &[(&str, Tag)]| -> Compound { pairs.iter().map(|(k, v)| ((*k).into(), v.clone())).collect() };
    let look = |name, d: &Compound| {
        mob_look(name, d)
            .into_iter()
            .map(|(l, t, _)| (l, t))
            .collect::<Vec<_>>()
    };
    let pair = |l: &str, t: &str| (l.to_owned(), t.to_owned());
    assert_eq!(look("cow", &data(&[])), [pair("cow", "entity/cow/cow_temperate")]);
    assert_eq!(
        look("cow", &data(&[("variant", string("minecraft:cold"))])),
        [pair("cold_cow", "entity/cow/cow_cold")]
    );
    assert_eq!(
        look(
            "cow",
            &data(&[("variant", string("minecraft:warm")), ("Age", Tag::Int(-2000))])
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
        ("sheep#wool".to_owned(), "entity/sheep/sheep_wool".to_owned(), 0xB02E26)
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

#[test]
#[ignore]
fn every_mob_has_a_look() {
    use mod_api::serde_json::Value;
    let root = std::path::PathBuf::from(std::env::var("HORIZON_RESOURCES").expect("HORIZON_RESOURCES"));
    let read =
        |name: &str| -> Value { mod_api::serde_json::from_slice(&std::fs::read(root.join(name)).unwrap()).unwrap() };
    let (types, models) = (read("entity-types.json"), read("entity-models.json"));
    let mut missing = Vec::new();
    for (kind, info) in types.as_object().unwrap() {
        let living = info
            .get("attributes")
            .is_some_and(|a| a.as_object().is_some_and(|a| !a.is_empty()));
        if !living
            || info["category"] == "monster"
            || ["minecraft:player", "minecraft:armor_stand", "minecraft:mannequin"].contains(&kind.as_str())
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
                let file = |t: &str| root.join(format!("assets/minecraft/textures/{t}.png")).exists();
                if !file(&texture) && !file(&texture.replace("_baby", "")) {
                    missing.push(format!("{kind}: texture {texture}"));
                }
            }
        }
    }
    missing.dedup();
    assert!(missing.is_empty(), "{} missing:\n{}", missing.len(), missing.join("\n"));
}
