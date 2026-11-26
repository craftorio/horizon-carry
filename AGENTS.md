# AGENTS.md

Carry (`craftorio_carry`): a Horizon mod inspired by the Java mod Carry On. Horizon is Minecraft 26.3
rewritten in Rust (our fork: `craftorio/horizon`); this mod is Rust built to WebAssembly (`.hmod`) and runs on the
Horizon server and on every client. Sneak with both hands empty and right-click a block with contents or a
peaceful mob: it leaves the world and the player carries it (slowed, hands full, drawn in first and third person
with the arms forward); a right click on a block puts it down; dying puts it down where the player died.

## Layout

No Rust file is longer than 400 lines (`scripts/max-lines.sh`, part of `make check`). Split a longer one into
modules.

- `src/lib.rs`: the hooks. They only call the modules. `prelude` is what every module uses.
- `src/carried.rs`: what a player carries (`Carried`), the `CARRIED` attachment and its codec.
- `src/state.rs`: block states as text, and how a carried block is put down (facing, double chests).
- `src/config.rs`: the rules, `craftorio_carry-server.toml`, with its parser. The default text is
  `examples/craftorio_carry-server.toml`. It is a `Server` config: a dedicated server and the built-in server of
  single player both read it.
- `src/server/`: the server decides every pick-up and put-down (`pick.rs`, `place.rs`), puts down into water on
  the client's `PutDown` message (`water.rs`), slows a carrying player (`slowdown.rs`), and puts down what a
  dying player carries (`mod.rs`).
- `src/client/`: the client answers clicks while its player carries something, and reads what each player in
  view carries once a tick (`mod.rs`); it draws the load and the arms (`draw.rs`, `skin.rs`).
- `src/client/look/`: the model layers and textures of each vanilla mob, by group. A mod cannot ask the game to
  draw an entity in a 3D pass, so the rules of the vanilla renderers are written here. `tests.rs` checks every
  living type that is not a monster against the game's resources:
  `HORIZON_RESOURCES=<horizon-prepare output> cargo test every_mob -- --ignored`.
- `horizon-mod.toml`: the manifest. `version` is compared at join: a Horizon server lets in only clients with
  exactly its mods and versions, so bump it with every change and ship client and server together.

## Building and shipping

The mod API is the path `../horizon/crates/horizon-mod-api`: a `craftorio/horizon` checkout beside this one, at the
commit the shipped game is built from. There is no published Horizon SDK, and the mod tool builds a wasm mod
without one only inside a game checkout, so the WebAssembly build is done by craftorio-launcher's
`make horizon-mods` (`scripts/horizon-mods.sh`): it copies this folder into `<horizon>/target/craftorio-mods/`,
builds it there and puts `horizon-mods/craftorio_carry.hmod` into the Horizon client of the bundles repository.
Then `make publish-client CLIENT=Horizon REQUIRES=` there, and the same `.hmod` into the server's
`horizon-mods/` (with `mod.craftorio_carry:true` in its `horizon-mods.txt`) and a restart. See "Моды Horizon" in
craftorio-launcher's `docs/decisions.md`.

A release is a tag: bump `version` in `horizon-mod.toml` and `Cargo.toml`, commit, tag `v<version>` and push the
tag. `.github/workflows/release.yml` checks the tag against the manifest, runs the checks, builds against the
`craftorio/horizon` commit in `HORIZON_COMMIT` (keep it the shipped game's: `docker/26.3-horizon/HORIZON_COMMIT`
of `craftorio/docker-server`) and puts `craftorio_carry.hmod` on the tag's release. It reads the private Horizon
repository with the `HORIZON_DEPLOY_KEY` secret (a read-only deploy key of `craftorio/horizon`). Pushes without a
tag build nothing.

Before shipping, start a client once with the new `.hmod` and check its log says `wasm mods loaded (1)`: a
client-only call the game refuses would keep every player out of the server, and the server's own load does not
show it.

## Rules

- All documentation is in Simplified English (in the spirit of ASD-STE100): `AGENTS.md`, `README.md`, doc
  comments and comments in code, commit messages. Short sentences, one idea in each. Common words, and one word
  for one thing. Active voice. No idioms.

- Checks before finishing: `make check` (= `scripts/max-lines.sh`, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`). `make hooks` once per clone: `pre-commit` runs them, `commit-msg` rejects a `Co-authored-by`
  trailer.
- Commit only when asked. Never add `Co-Authored-By` (or any other AI attribution line) to commit messages.
- Look up the mod API in the checkout, not from memory: hook declarations in
  `../horizon/crates/horizon-mod-vocab/src/hooks/`, host methods in `.../src/host/`, the generated event structs in
  `../horizon/target/*/build/horizon-mod-vocab/*/out/mod_api_generated.rs` (after any build of the game), the
  guides in `../horizon/docs/modding/src/`.
- Visual numbers (offsets, angles, sizes in `render_hand`, `CarriedLayer`, `render_player_pre`) were tuned by
  the owner looking in game: change them only with a reason seen in game.
