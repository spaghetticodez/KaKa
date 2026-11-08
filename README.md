<div align="center">
  <img src="image/kaka.png" width="120" alt="KaKa logo">
  <h1>KaKa Trainer Launcher</h1>
  <p>Run your own Windows game trainers inside the Proton prefix of the game you are playing — on Linux.</p>
</div>

KaKa is a **Linux-only, offline** fork of [LaLa Trainers Launcher](https://github.com/wyyadd/LaLa).
It has no online catalogue, no update checks and no telemetry: you add trainer `.exe` files you
already have, KaKa finds the running Proton game and starts the trainer in the same Wine prefix.

## How it works

1. Start your game (Steam, Heroic, Lutris, umu-launcher — anything that runs it with Proton).
2. In KaKa, click **+**, pick the trainer `.exe`, give it a name (optional cover image).
3. Click the trainer card. KaKa:
   - finds the running game through its `STEAM_COMPAT_DATA_PATH` (works for any library drive and
     non-Steam shortcuts),
   - uses the same Proton build the game was started with (or the one recorded in the prefix's `config_info`),
   - runs the trainer with `proton runinprefix` (Wine directly in the game's prefix, joining the game's
     wineserver — also when Steam runs the game inside the Steam Linux Runtime container), from the
     trainer's own folder. Older Proton builds without `runinprefix` fall back to `proton run`,
   - writes a log to `~/.local/state/kaka/logs/` and shows the error output if the trainer exits immediately.

**Steam AppID (optional, per trainer)** — only needed when several Proton games run at once; KaKa then
picks the game with that AppID. See [how to find the AppID](docs/how_to_get_appid.md).

Always start the game first. A trainer started before the game keeps the prefix's Wine server alive,
and Steam waits for that server to exit before it starts the game — the game would hang until the
trainer closes. KaKa therefore refuses to launch a trainer when its game is not running.

When KaKa is closed, it stops the trainers it started (only those — the game and its Wine services keep running).

## Build

Requires the Flutter version pinned in [`.fvmrc`](.fvmrc) and the usual Linux desktop toolchain
(`clang cmake ninja pkgconf gtk3`).

```sh
flutter pub get
flutter analyze
flutter test
flutter build linux --release
./build/linux/x64/release/bundle/kaka
```

Packaging (AppImage / Flatpak / Arch package) is not decided yet; the files in `dist/` are legacy
LaLa recipes and are not maintained.

## Data

| What | Where |
|---|---|
| Library and settings | `$XDG_DATA_HOME/io.github.spaghetticodez.kaka/` (`~/.local/share/...`) |
| Launch logs (last 20) | `$XDG_STATE_HOME/kaka/logs/` (`~/.local/state/...`) |

On first start, custom trainers from an existing LaLa install (`~/.cache/com.aironheart.lala/config`)
are imported once. LaLa online-catalogue entries are skipped. The LaLa files are not modified.

## Troubleshooting

- **"No running Proton game found"** — start the game first and [make sure it uses Proton](docs/enable_proton.md).
- **Trainer says "game not detected"** — the trainer probably does not match the game version.
  Try a trainer build for your game version.
- **"The game is still starting"** — the game is still setting up its Proton prefix (first start or after a
  Proton update). Wait a few seconds and launch again.
- **Steam path in Settings** — normally leave it on auto-detect. It is only used when the game's launcher
  did not tell KaKa where Steam is; KaKa checks `~/.steam/root`, `~/.local/share/Steam`, `~/.steam/steam`,
  Debian, Flatpak and Snap Steam locations automatically.
- **Some trainers will not start** — often a .NET issue in the prefix, see [reinstalling dotnet40](docs/reinstall_dotnet40.md).
- **Trainer window flickers** — Wayland issue; use X11 or [enable a Wine virtual desktop](docs/enable_virtual_desktop.md).
- **"Previous instance did not exit properly"** (FLiNG trainers) — delete the trainer's temp folders in the
  prefix: `pfx/drive_c/users/steamuser/AppData/Local/Temp/` and `.../AppData/Local/FLiNGTrainer/`.
- Anything else: check the newest log in `~/.local/state/kaka/logs/`.

## Safety

Trainers are Windows programs that modify game memory. KaKa runs whatever `.exe` you add, with your
user's permissions (Wine maps your whole filesystem as drive `Z:`). Only add trainers from sources you
trust, and never use them in online or anti-cheat protected games.

## License and credits

AGPL-3.0, see [LICENSE](LICENSE). Based on [wyyadd/LaLa](https://github.com/wyyadd/LaLa) by wyyadd.
