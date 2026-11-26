//! Farm animals and pets: cows, pigs, chickens, sheep, wolves, cats, ocelots, rabbits, parrots.

use super::{Look, Mob, dye, nth};
use crate::prelude::*;

pub(super) fn look(m: &Mob) -> Option<Look> {
    let (name, b) = (m.name, m.b);
    Some(match name {
        "cow" | "pig" | "chicken" => {
            let v = m.v().unwrap_or("temperate");
            // cold ones have a model of their own, and so has a warm cow
            let layer = match (name, v) {
                ("cow", "warm") | (_, "cold") if !m.baby => format!("{v}_{name}"),
                ("cow", "warm") | ("cow", "cold") => format!("{v}_{name}{b}"),
                _ => format!("{name}{b}"),
            };
            m.one(layer, format!("entity/{name}/{name}_{v}{b}"))
        }
        "mooshroom" => {
            let kind = read_string(m.data, "Type").unwrap_or_else(|| m.v().unwrap_or("red").to_owned());
            m.one(format!("mooshroom{b}"), format!("entity/cow/mooshroom_{kind}{b}"))
        }
        "sheep" => {
            let mut look = m.one(format!("sheep{b}"), format!("entity/sheep/sheep{b}"));
            if int(m.data, "Sheared") != Some(1) {
                let color = dye(int(m.data, "Color").unwrap_or(0).into());
                look.push((format!("sheep{b}#wool"), format!("entity/sheep/sheep_wool{b}"), color));
            }
            look
        }
        "wolf" => {
            let tame = if m.data.contains_key(&"Owner".into()) {
                "_tame"
            } else {
                ""
            };
            let texture = match m.v() {
                None | Some("pale") => format!("entity/wolf/wolf{tame}{b}"),
                Some(v) => format!("entity/wolf/wolf_{v}{tame}{b}"),
            };
            m.one(format!("wolf{b}"), texture)
        }
        "cat" => m.one(
            format!("cat{b}"),
            format!("entity/cat/cat_{}{b}", m.v().unwrap_or("tabby")),
        ),
        "ocelot" => m.one(format!("ocelot{b}"), format!("entity/cat/ocelot{b}")),
        "rabbit" => {
            let kind = m.v().map(str::to_owned).unwrap_or_else(|| {
                match int(m.data, "RabbitType").unwrap_or(0) {
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
            m.one(format!("rabbit{b}"), format!("entity/rabbit/rabbit_{kind}{b}"))
        }
        "parrot" => {
            const COLORS: [&str; 5] = ["red_blue", "blue", "green", "yellow_blue", "grey"];
            let color = m.v().unwrap_or_else(|| nth(&COLORS, int(m.data, "Variant")));
            m.one("parrot", format!("entity/parrot/parrot_{color}"))
        }
        _ => return None,
    })
}
