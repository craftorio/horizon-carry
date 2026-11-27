# Tasks

## 1. The arms through JEA

- [x] 1.1 The clip `assets/craftorio_carry/animations/hold.animation.json`: the arms forward by Carry's old angles.
- [x] 1.2 `src/server/arms.rs`: play and stop through `/jea` with `run_command`, on each change of "carries
      something", after a login too; a test of the command lines.
- [x] 1.3 The client: no arms in third person (`draw::player` and the `render_player_pre` hook go); `skin.rs` keeps
      only the arm parts for first person.
- [x] 1.4 The client reads the loads of all players from the synced attachment (`query`), not from the tab list: a
      LAN guest did not see the host's load.
- [x] 1.5 README and AGENTS.md: the arms need JEA; without it they hang. The version 0.6.0.

## 2. Checks and shipping

- [x] 2.1 `make check` and `openspec validate --strict`.
- [x] 2.2 In game: a native client with JEA and Knockout, the new `.hmod`, LAN with two clients. The host carries a
      chest and a mob in full armour; the guest sees the arms forward with the armour, the crouch, and the arms
      back after the put-down. Screenshots in `horizon-work/screenshots/09-carry-arms/`. Also
      checked: a login with a load plays the clip again; a death stops it and puts the cow down.
- [ ] 2.3 Ship: a tag `v0.6.0` (the release workflow builds the `.hmod`), then craftorio-launcher's
      `make horizon-mods` and `make publish-client CLIENT=Horizon REQUIRES=`, the same `.hmod` on the server and a
      restart. The server must have JEA.
- [ ] 2.4 On the dev server: see a carrying player from another client.
