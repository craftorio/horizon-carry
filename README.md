# Carry

A mod for [Horizon](https://playhorizon.dev) (Minecraft 26.3 in Rust). It does what the Java mod Carry On does:
you can carry blocks with contents and peaceful mobs.

Sneak with both hands empty. Right-click a chest, a furnace, a barrel or a cow. It goes out of the world, and you
carry it in front of you. Right-click a block to put it down. A block keeps all its contents.

The rules are in `craftorio_carry-server.toml`. They say which blocks and mobs you can pick up, if monsters and
the pets of other players are allowed, and how much slower you walk. The game writes the file when it is not
there. The rules are for each world, on a server and in single player (the world's own built-in server):

- in `<world>/serverconfig/craftorio_carry-server.toml` for one world (`saves/<world>/` in single player);
- else in `config/craftorio_carry-server.toml` of the server or the game folder, for all worlds.

The game reads the file again when it changes. [AGENTS.md](AGENTS.md) tells how to build and release the mod.

License: MIT.
