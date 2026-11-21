# Cheat Engine tables (.CT)

KaKa opens a Cheat Engine table in the game you are playing, using **your own** copy of Cheat Engine.
KaKa never downloads Cheat Engine or tables, and never reads or writes game memory itself.

## Setup

1. Download Cheat Engine **for Windows** from cheatengine.org (GitHub has no binaries). Use the Windows
   build even on Linux: KaKa runs it inside the game's Proton prefix, where Wine lets it read the game's
   memory without changing any system security setting. The native Linux build of Cheat Engine is not
   supported: it needs `kernel.yama.ptrace_scope=0` or root to read game memory. If the site only offers
   the newest version for Linux and Mac, the Windows build is on Cheat Engine's Patreon or follows later.
   The installer offers extra software: decline it. Install it with Proton into any spare prefix, for example:

   ```sh
   mkdir -p ~/.local/share/kaka-ce
   STEAM_COMPAT_DATA_PATH=~/.local/share/kaka-ce \
   STEAM_COMPAT_CLIENT_INSTALL_PATH=~/.local/share/Steam \
   ~/.local/share/Steam/steamapps/common/"Proton - Experimental"/proton run ~/Downloads/CheatEngine*.exe
   ```

2. In KaKa, click **+**, pick the table under **Cheat Engine table**, and choose `cheatengine-x86_64.exe`
   from the Cheat Engine folder as the trainer (not `Cheat Engine.exe`, which only starts the other one
   and exits). After your first table, KaKa fills Cheat Engine in for you. KaKa shows how many values and
   scripts the table has before you save it.
3. Start the game, then click **Launch**. Cheat Engine opens with the table. If the table does not
   attach by itself, click the computer icon in Cheat Engine and pick the game's process.

## Safety

- Single-player games only. Using Cheat Engine with online games or anti-cheat (EasyAntiCheat,
  BattlEye) can get your account banned.
- Tables are programs. Auto Assembler and Lua scripts run with your user rights, and Wine maps your
  whole filesystem as drive `Z:`: it is not a sandbox. Use tables from sources you trust.
- Cheat Engine asks before it runs a table's Lua script (unless the table is signed). **Do not choose
  "always execute"** in that question: it applies to every table you open later.
- A `{$lua}` block inside an Auto Assembler script runs as soon as you switch that script on, without
  a question.
- Cheat Engine also loads `.lua` files from the table's folder: keep each table in its own folder.
- Tables are made for one game version. After a game update, entries can show `??` until the table
  author updates the table.
