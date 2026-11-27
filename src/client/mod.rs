//! The client: what each player in view carries, read once a tick from the synced attachments, for the render
//! hooks (they get no host to read it with).

use crate::prelude::*;

pub(crate) mod draw;
pub(crate) mod look;
pub(crate) mod skin;

/// What each player in view carries, by their entity id, and this client's own player's id.
#[derive(Default)]
pub(crate) struct ClientView {
    pub(crate) local: Option<u32>,
    pub(crate) shown: HashMap<u32, (Carried, Shown)>,
    /// Baked model layers by name (`minecraft:cow#main`), None for a layer the game hasn't.
    pub(crate) layers: HashMap<String, Option<Arc<ModelPart>>>,
    /// The arm parts of the player model, for this client's own player in first person: [`Arms`].
    pub(crate) arms: HashMap<u32, Arms>,
    /// Which of them crouch: their load goes down and forward with the body, as vanilla's crouch moves the arms.
    pub(crate) crouching: HashSet<u32>,
}

/// How a crouch moves the load in front of a player.
pub(crate) const CROUCH_LOAD: [f32; 3] = [0.0, -0.2, 0.12];

pub(crate) static VIEW: Mutex<Option<ClientView>> = Mutex::new(None);

pub(crate) fn view() -> std::sync::MutexGuard<'static, Option<ClientView>> {
    VIEW.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// What the client does with a click while its player carries something: nothing of its own (no block's use,
/// no item's), the server puts the thing down. Picking up is the server's alone: the client lets the click go
/// to it as usual (a chest opens only on the server's word), so the rules live on the server only.
pub(crate) fn client_carrying(host: &mut dyn ClientHost) -> bool {
    host.local_player().is_some_and(|player| {
        CARRIED
            .get_existing(host, Holder::Player(player.uuid()))
            .ok()
            .flatten()
            .flatten()
            .is_some()
    })
}

/// Reads what every player in this client's level carries: the rows of the synced attachment. A LAN guest did not
/// see the load of the host when this read the players of the tab list.
pub(crate) fn tick(host: &mut dyn ClientHost) {
    let local = host.local_player();
    let mut carried_by = Vec::new();
    if let Ok(batch) = host.query(&CARRIED) {
        for row in batch.iter().flatten() {
            if let (AttachmentHolder::Player(player), Some(what)) = (row.holder, row.value.clone()) {
                carried_by.push((player, what));
            }
        }
    }
    let mut view = view();
    let view = view.get_or_insert_with(ClientView::default);
    view.local = local.map(|player| player.entity().id());
    let (mut shown, mut arms) = (HashMap::new(), HashMap::new());
    view.crouching.clear();
    for (player, what) in carried_by {
        let id = player.entity().id();
        if player.entity().is_crouching(host) == Some(true) {
            view.crouching.insert(id);
        }
        if view.local == Some(id)
            && let Some(held) = view
                .arms
                .remove(&id)
                .or_else(|| arms_of(host, &mut view.layers, player))
        {
            arms.insert(id, held);
        }
        let known = view.shown.remove(&id).filter(|(was, _)| *was == what);
        let entry = known.or_else(|| draw::shown(host, &mut view.layers, &what).map(|look| (what, look)));
        if let Some(entry) = entry {
            shown.insert(id, entry);
        }
    }
    view.shown = shown;
    view.arms = arms;
}
