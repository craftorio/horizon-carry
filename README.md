# Carry

A mod for [Horizon](https://playhorizon.dev) (Minecraft 26.3 in Rust). It does what the Java mod Carry On does:
you can carry blocks with contents and peaceful mobs.

Sneak with both hands empty. Right-click a chest, a furnace, a barrel or a cow. It goes out of the world, and you
carry it in front of you. Right-click a block to put it down. A block keeps all its contents.

The server rules are in `config/craftorio_carry-server.toml`. The server writes this file when it starts for the
first time. The rules say which blocks and mobs you can pick up, if monsters and the pets of other players are
allowed, and how much slower you walk. [AGENTS.md](AGENTS.md) tells how to build and release the mod.

License: MIT.
