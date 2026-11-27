//! Drawing what is carried: as an item (a block) or as a model (a mob), in the hands and in front of a player.

use crate::prelude::*;

/// How a carried thing is drawn: a block as its item (a chest's or a furnace's own 3D model), a mob as its model
/// (its variant's texture), or, for a mob whose model or texture isn't found, its spawn egg.
#[derive(Clone)]
pub(crate) enum Shown {
    Item(ItemStack),
    Mob {
        parts: Vec<(Arc<ModelPart>, String, u32)>,
        egg: Option<ItemStack>,
    },
}

pub(crate) fn item_of(host: &dyn Host, id: &str) -> Option<ItemStack> {
    Some(ItemStack::of(host.item_key(&Identifier::parse(id).ok()?)?))
}

pub(crate) fn layer(
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

pub(crate) fn shown(
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
                    let part =
                        layer(host, layers, &model).or_else(|| layer(host, layers, &model.replace("_baby", "")))?;
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
pub(crate) fn wiggle(part: &mut ModelPart, time: f32) {
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
pub(crate) fn draw_shown(draw: &mut dyn BlockEntityDrawContext, shown: &Shown, size: f32) {
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
pub(crate) struct CarriedLayer;

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

/// In first person, the carried thing is held in front with both hands, in place of the arms: below the view,
/// tilted so its top shows, the arms along its sides in the player's own skin.
pub(crate) fn hand(host: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
    let (shown, arms) = {
        let view = view();
        let view = view.as_ref();
        let get = |id| view.and_then(|view| Some((view.shown.get(&id)?.1.clone(), view.arms.get(&id).cloned())));
        match view.and_then(|view| view.local).and_then(get) {
            Some(found) => found,
            None => return Flow::Continue,
        }
    };
    if event.render.hand == Hand::Main {
        let draw = &mut *event.draw;
        let low = -0.62 - event.render.equip * 0.4;
        if let (Some(arms), Some((sprite, _))) = (arms, host.local_player_arm_skin()) {
            for (part, side) in [(&arms.right, 1.0f32), (&arms.left, -1.0)] {
                let mut part = (**part).clone();
                (part.x, part.y, part.z, part.x_rot, part.y_rot, part.z_rot) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
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
