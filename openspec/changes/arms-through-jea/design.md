# Design: the arms through JEA

## The command

A WebAssembly mod cannot link JEA's Rust API. So Carry runs JEA's command with `run_command`. The command runs as
the console (level 4), and JEA asks for level 2. JEA's own checks ran a native command from a WebAssembly hook
(`openspec/changes/animation-core/design.md` of `craftorio/horizon-jea`, question 4).

```
jea play <uuid> craftorio_carry:hold craftorio_carry:main 1500 3 loop
jea stop <uuid> craftorio_carry:main 3
```

- The target is the player's UUID: a player name can change, a UUID cannot.
- Priority 1500 is in JEA's band of side states (1000-1999). A Horizon emote (2500) and Knockout (3000) play over
  it.
- Fades of 3 ticks, in and out. `loop`: the clip plays until it is stopped.
- The command has no mask and no mode: JEA uses all bones and Replace. The clip has tracks for the two arms only,
  and JEA moves only the bones of a clip. So the clip itself is the mask.

## Replace, not Add

Replace sets the rotation of an arm to the clip's value. It keeps the position that vanilla gives. So:

- the arms do not swing while the player walks (they hold the load);
- a crouch still moves the arms down with the body, because the clip has no position track. Before, Carry moved its
  own arms down by the same 3.2 texels.

Add would put the walking swing on top of the clip. The load does not swing, so the arms would move through it.

## When the server tells JEA

`player_tick_post` compares, for each player, "carries something" with what Carry last told JEA. It runs a command
only when they differ. The same pattern as the slowdown (`slowdown.rs`):

- Picking up, putting down, putting down into water and death all change the carried attachment. The next tick
  tells JEA. One place handles all of them.
- After a login or a server restart, Carry has told JEA nothing. JEA saves its animation with the player. So the
  first tick plays the clip again (the player still carries) or stops it (the load is gone).
- `player_logged_out` forgets the player.
- Carry asks once if the server has the command `jea` (`command_info`). Without it, Carry runs nothing. A command
  that fails writes a warning to the server log.

## The clip

The angles are the ones that the owner tuned in game for Carry's own arms: up from hanging by 0.95 radians and in
by 0.15 radians. JEA keeps the signs of a Bedrock rotation (`[-90, 0, 0]` points an arm forward), so the clip has
`right_arm` `[-54.43, -8.59, 0]` and `left_arm` `[-54.43, 8.59, 0]` (degrees).
