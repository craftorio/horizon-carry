# Carry

Carry is a mod for [Horizon](https://playhorizon.dev), Minecraft Java Edition 26.3 in Rust. It is inspired by
the Java mod [Carry On](https://modrinth.com/mod/carry-on): you can pick up a block with its contents, or a
peaceful mob, and carry it to a different place.

## How to play

1. Sneak, and keep both hands empty.
2. Right-click a block that holds something (a chest, a furnace, a barrel) or a peaceful mob (a cow, a wolf, a
   villager). It goes out of the world, and you carry it in front of you.
3. Right-click a block to put it down next to that block. Right-click water to put a fish or a squid into it.

What you carry keeps all its data: the items in a chest, the name and the health of a mob, the owner of a pet.

While you carry something:

- you walk a quarter slower;
- your hands are full: you cannot use items, hit, dig, change the hotbar slot or trade;
- you see the load in your hands, and other players see it in front of you, with your arms forward (see
  below);
- if you die, the load stays where you died.

A mob is put down only where its whole body fits. You cannot pick up a chest while a player looks into it.

## The arms

The arms of a carrying player go forward through the mod
[Just Enough Animations](https://github.com/craftorio/horizon-jea) (JEA, the mod `jea`). The server plays the
clip `craftorio_carry:hold` on the player (`assets/craftorio_carry/animations/hold.animation.json`), so the armour
on the arms turns with them. The server and the clients need JEA for this. Without JEA, the arms of a carrying
player hang. The load is still in front of them, and the rest of Carry works as usual. In first person, Carry
draws the arms itself, so you see them without JEA too.

## Rules

The rules are in `craftorio_carry.toml`. The game writes this file with the default rules when it is not
there. The default file, with a comment for each rule, is
[`examples/craftorio_carry.toml`](examples/craftorio_carry.toml).

You can set:

- which blocks you can pick up: only blocks with contents, or all blocks that can be broken;
- lists of blocks and mobs that you always can, or never can, pick up (an id can have one `*`);
- if monsters, and the pets of other players, can be picked up;
- how much slower a player walks with a load.

The part of the game that runs the world reads the rules: a dedicated server, or in single player the
built-in server of your game. So the rules are the same in both, and they are for each world:

- `<world>/serverconfig/craftorio_carry.toml` is for one world (`saves/<world>/` in single player);
- else `config/craftorio_carry.toml` of the server or the game folder is for all worlds.

The game reads the file again when it changes. If the file has an error, the old rules stay, and the server log
tells the error.

## Install

On Craftorio, the launcher installs and turns on the mod. On a different Horizon install:

1. Get `craftorio_carry.hmod` from the [releases](https://github.com/craftorio/horizon-carry/releases).
2. Put it into the `horizon-mods/` folder of the game folder (the folder with `options.txt`), and of the server.
3. Add the line `mod.craftorio_carry:true` to `horizon-mods.txt` in the same folder.

The server and every player must have the same version of the mod. A Horizon server does not let in a player
with different mods. The mod is WebAssembly: the game must have `horizon-wasm-std.wasm` next to its program.

Each release is built for one Horizon build: the commit of
[craftorio/horizon](https://github.com/craftorio/horizon) in [`HORIZON_COMMIT`](HORIZON_COMMIT).

## Build

You need a checkout of `craftorio/horizon` next to this one, as `../horizon`. Then:

```sh
make hooks   # once: the checks before each commit
make check   # file length, formatting, clippy and tests
```

To make a release, change `version` in `horizon-mod.toml` and `Cargo.toml`, push the commit, and push the tag
`v<version>`. The release workflow builds `craftorio_carry.hmod` and attaches it to the release.
[AGENTS.md](AGENTS.md) tells more about the code and the release.

## License

MIT, see [LICENSE](LICENSE). Carry is a new mod written in Rust. It does not use the code or the assets of
Carry On.
