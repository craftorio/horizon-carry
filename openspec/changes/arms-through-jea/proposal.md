# Carry: the arms of a carrying player through Just Enough Animations

## Why

On the dev server, the owner saw a carrying player from the side (2026-11-27). The arms had strange textures, and
the armour sleeves did not turn with the arms. The cause: Carry draws its own arms forward with the skin sprite from
the atlas, and hides the arms of the model. The armour layer of the game stays on the model's arms, which hang.

Just Enough Animations (JEA, the mod `jea`) poses the model itself. The armour, the held items and the cape follow
its pose. So Carry asks JEA to hold the arms forward, and does not draw arms in third person (the owner's decision,
2026-11-27).

## What changes

- The server plays the clip `craftorio_carry:hold` on a player when they pick something up, on the channel
  `craftorio_carry:main`, with JEA's command (`run_command`). It stops the channel when the player puts the load
  down, when the load goes down at their death, and when a player logs in without a load (JEA saves its animation
  with the player).
- The clip is a file: `assets/craftorio_carry/animations/hold.animation.json`. The arms point forward by the same
  angles as before, and a little in towards each other.
- The client does not draw the arms in third person any more. It still draws the load in front of the player, and
  it keeps the first-person view as it is.
- The client reads what each player carries from the rows of the synced attachment (`query`), not from the
  players of the tab list. In the LAN test of this change, a guest did not see the load of the host (and so, before
  this change, not its arms either) when the client read the tab list.
- Without JEA on the server or on a client, the arms of a carrying player hang. The README says so.

## Non-goals

- First-person arms through JEA. JEA does not animate first person.
- A dependency on JEA in `horizon-mod.toml`. Carry is a WebAssembly mod and uses only JEA's command, so Carry still
  loads without JEA.
- New poses (walking with a load, a heavy load). Later, as more clips.
