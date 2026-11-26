//! Carry: a Horizon mod inspired by the Java mod Carry On. Sneak with both hands empty and right-click a block
//! that holds something (a chest, a furnace, a barrel...) or a peaceful mob. It goes out of the world, contents
//! and all, and the player carries it, slowed down. A right click on a block puts it back next to that block (or
//! in its place when it is replaceable, as grass is). Dying puts it down where the player died.
//!
//! The server decides (`server`). The client answers the same clicks so that it does not open the chest that it
//! is about to lose, and it draws what is carried (`client`). What a player carries is an attachment on the
//! player (`carried`): saved with them and synced to their client. The hooks here only call the modules.

mod carried;
mod client;
mod config;
mod server;
mod state;

/// What every module uses: the mod API, the shared helpers of this crate, and std.
mod prelude {
    pub(crate) use crate::carried::*;
    pub(crate) use crate::client::draw::*;
    pub(crate) use crate::client::look::*;
    pub(crate) use crate::client::skin::*;
    pub(crate) use crate::client::*;
    pub(crate) use crate::server::pick::*;
    pub(crate) use crate::server::place::*;
    pub(crate) use crate::server::slowdown::*;
    pub(crate) use crate::state::*;
    pub(crate) use crate::{NAMESPACE, config};
    pub(crate) use mod_api::{
        AttachmentCodec, AttachmentError, AttachmentType, AttackEntityEvent, AttributeModifier, AttributeOperation,
        BlockClick, BlockEntityDrawContext, BlockState, BlockUpdateFlags, BreakBlockEvent, ClientHost,
        ClientTickPostEvent, Compound, Config, ConfigFile, ConfigKind, ConfigLoadingEvent, ConfigReloadingEvent,
        ConsoleLevel, EntityInteractEvent, EntityRef, EntityRenderLayer, EntityRenderState, Flow, GameMode, Hand,
        Holder, Hooks, Host, HostBlockStates, HostSide, HumanoidPart, Identifier, InteractionResult,
        ItemDisplayContext, ItemStack, LayerTarget, LevelRef, LivingDeathEvent, ModelPart, ModifierLifetime,
        PayloadCodec, PayloadId, PlayerLoggedOutEvent, PlayerRef, PlayerSwitchHotbarSlotPreEvent, PlayerTickPostEvent,
        PosedParts, RegisterConfigsEvent, RegisterEntityLayersEvent, Registrar, RegistrationContext, RenderHandEvent,
        RenderPlayerPreEvent, RightClickBlockEvent, RightClickEmptyEvent, RightClickItemEvent, ServerHost, SoundSource,
        Tag, ToServer, VarInt,
    };
    pub(crate) use std::collections::{HashMap, HashSet};
    pub(crate) use std::sync::{Arc, Mutex};
}

use prelude::*;

pub(crate) const NAMESPACE: &str = "craftorio_carry";

/// Cancels an event that the hands of a carrying player cannot do.
fn hands_full(carrying: bool, cancelled: &mut bool) -> Flow {
    if !carrying {
        return Flow::Continue;
    }
    *cancelled = true;
    Flow::Cancel
}

#[derive(Default)]
pub struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, registrar: &mut dyn Registrar, _context: &RegistrationContext) {
        server::water::register(registrar);
    }

    /// A click at nothing (water is nothing to aim at) while carrying: the server looks for water to put down in.
    fn right_click_empty(&self, host: &mut dyn ClientHost, event: &mut RightClickEmptyEvent) {
        if event.hand == Hand::Main {
            server::water::click_at_nothing(host);
        }
    }

    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let click = event.click;
        if click.hand != Hand::Main {
            return Flow::Continue;
        }
        let answer = match host {
            HostSide::Server(host) => server_click(host, click),
            HostSide::Client(host) => client_carrying(host).then_some(InteractionResult::Success),
        };
        let Some(result) = answer else { return Flow::Continue };
        *event.cancellation_result = result;
        event.cancelled = true;
        Flow::Cancel
    }

    fn entity_interact(&self, host: HostSide, event: &mut EntityInteractEvent<'_>) -> Option<InteractionResult> {
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
        server::take_config(host, event.config);
    }

    fn config_reloading(&self, host: HostSide, event: &mut ConfigReloadingEvent<'_>) {
        server::take_config(host, event.config);
    }

    /// Hands full: no item is used while carrying (a click on a block puts the load down instead).
    fn right_click_item(&self, host: HostSide, event: &mut RightClickItemEvent<'_>) -> Flow {
        let carrying = match host {
            HostSide::Server(host) => carried(host, event.click.player).is_some(),
            HostSide::Client(host) => client_carrying(host),
        };
        if carrying {
            *event.cancellation_result = InteractionResult::Fail;
        }
        hands_full(carrying, &mut event.cancelled)
    }

    /// Hands full: no hitting.
    fn attack_entity(&self, host: &mut dyn ServerHost, event: &mut AttackEntityEvent<'_>) -> Flow {
        hands_full(acting_carries(host), &mut event.cancelled)
    }

    /// Hands full: no digging.
    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        hands_full(acting_carries(host), &mut event.cancelled)
    }

    /// Hands full: the hotbar stays on its slot, so nothing else comes into the hand.
    fn player_switch_hotbar_slot_pre(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerSwitchHotbarSlotPreEvent,
    ) -> Flow {
        hands_full(carried(host, event.player).is_some(), &mut event.cancelled)
    }

    fn player_tick_post(&self, host: HostSide, event: &mut PlayerTickPostEvent) {
        if let HostSide::Server(host) = host {
            update_slowdown(host, event.player);
        }
    }

    fn player_logged_out(&self, _host: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        forget_slowdown(event.player);
    }

    fn living_death(&self, host: &mut dyn ServerHost, event: &mut LivingDeathEvent<'_>) -> Flow {
        server::death(host, event);
        Flow::Continue
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        client::tick(host);
    }

    fn render_player_pre(&self, _host: &dyn ClientHost, event: &mut RenderPlayerPreEvent<'_>) -> Flow {
        client::draw::player(event);
        Flow::Continue
    }

    fn register_entity_layers(&self, _context: &RegistrationContext, event: &mut RegisterEntityLayersEvent<'_>) {
        for model in event.layers.player_models() {
            let _ = event.layers.add(LayerTarget::Player(model), Arc::new(CarriedLayer));
        }
    }

    fn render_hand(&self, host: &dyn ClientHost, event: &mut RenderHandEvent<'_, '_>) -> Flow {
        client::draw::hand(host, event)
    }
}
