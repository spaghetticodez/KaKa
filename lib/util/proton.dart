/// Pure Steam / Proton discovery helpers.
///
/// No Flutter imports: everything here is unit-tested in `test/proton_test.dart`.
library;

import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;

/// Environment marker set on every trainer KaKa starts. It keeps trainers out
/// of game detection and lets KaKa stop exactly the processes it started.
const String trainerMarkerEnv = 'KAKA_TRAINER';

const Set<String> _protonVerbs = {'waitforexitandrun', 'run', 'runinprefix'};

/// A snapshot of one process from `/proc`.
class ProcessInfo {
  final int pid;
  final Map<String, String> env;
  final List<String> cmdline;

  const ProcessInfo(this.pid, this.env, this.cmdline);
}

/// A game currently running under Proton, grouped by its compat-data prefix.
class RunningGame {
  /// `STEAM_COMPAT_DATA_PATH` of the game (the directory that contains `pfx/`).
  final String compatDataPath;

  /// `STEAM_COMPAT_CLIENT_INSTALL_PATH` (the Steam install root), if the launcher set one.
  final String? clientInstallPath;

  /// Steam AppID, if the launcher exposed one.
  final int? appId;

  /// Absolute path of the `proton` script the game was started with, if seen.
  final String? protonScript;

  /// Human readable name (game executable name, or the prefix folder name).
  final String label;

  const RunningGame({required this.compatDataPath, required this.label, this.clientInstallPath, this.appId, this.protonScript});

  String get displayName => appId != null ? '$label (AppID $appId)' : label;
}

/// Parses a NUL separated `/proc/<pid>/environ` blob.
Map<String, String> parseEnviron(List<int> bytes) {
  final env = <String, String>{};
  for (final entry in utf8.decode(bytes, allowMalformed: true).split('\u0000')) {
    final eq = entry.indexOf('=');
    if (eq > 0) env[entry.substring(0, eq)] = entry.substring(eq + 1);
  }
  return env;
}

/// Parses a NUL separated `/proc/<pid>/cmdline` blob.
List<String> parseCmdline(List<int> bytes) {
  final parts = utf8.decode(bytes, allowMalformed: true).split('\u0000');
  if (parts.isNotEmpty && parts.last.isEmpty) parts.removeLast();
  return parts;
}

/// Reads every process of the current user that is readable under [procRoot].
/// Processes that vanish or belong to other users are skipped silently.
List<ProcessInfo> readProcesses({String procRoot = '/proc'}) {
  final result = <ProcessInfo>[];
  final List<FileSystemEntity> entries;
  try {
    entries = Directory(procRoot).listSync(followLinks: false);
  } on FileSystemException {
    return result;
  }
  for (final entry in entries) {
    final pid = int.tryParse(p.basename(entry.path));
    if (pid == null) continue;
    try {
      final env = parseEnviron(File(p.join(entry.path, 'environ')).readAsBytesSync());
      final cmdline = parseCmdline(File(p.join(entry.path, 'cmdline')).readAsBytesSync());
      result.add(ProcessInfo(pid, env, cmdline));
    } on FileSystemException {
      continue;
    }
  }
  return result;
}

int? _positiveInt(String? value) {
  final n = int.tryParse(value ?? '');
  return n != null && n > 0 ? n : null;
}

String _exeName(String arg) => p.basename(arg.replaceAll('\\', '/'));

/// Groups Proton game processes by `STEAM_COMPAT_DATA_PATH`.
///
/// Works for Steam games on any library drive, non-Steam shortcuts and other
/// launchers that use Proton (Heroic, umu-launcher, ...), because all of them
/// export the prefix in the environment. Trainers started by KaKa are ignored.
List<RunningGame> detectRunningGames(Iterable<ProcessInfo> processes) {
  final groups = <String, List<ProcessInfo>>{};
  for (final proc in processes) {
    if (proc.env.containsKey(trainerMarkerEnv)) continue;
    final compat = proc.env['STEAM_COMPAT_DATA_PATH'];
    if (compat == null || compat.isEmpty) continue;
    groups.putIfAbsent(p.normalize(compat), () => []).add(proc);
  }

  final games = <RunningGame>[];
  groups.forEach((compat, procs) {
    int? appId;
    String? client;
    String? proton;
    String? exe;
    for (final proc in procs) {
      appId ??= _positiveInt(proc.env['STEAM_COMPAT_APP_ID']) ?? _positiveInt(proc.env['SteamAppId']) ?? _positiveInt(proc.env['SteamGameId']);
      final c = proc.env['STEAM_COMPAT_CLIENT_INSTALL_PATH'];
      if (client == null && c != null && c.isNotEmpty) client = c;
      final cmd = proc.cmdline;
      for (var i = 0; i < cmd.length; i++) {
        if (proton == null && p.basename(cmd[i]) == 'proton' && i + 1 < cmd.length && _protonVerbs.contains(cmd[i + 1])) {
          proton = cmd[i];
        }
        if (exe == null && cmd[i].toLowerCase().endsWith('.exe')) {
          final name = _exeName(cmd[i]);
          // Skip Wine/Proton helper executables; we want the game's name.
          if (!_wineHelpers.contains(name.toLowerCase())) exe = name;
        }
      }
    }
    // A prefix with neither a proton invocation nor a Windows executable is not a running game.
    if (proton == null && exe == null) return;
    games.add(RunningGame(compatDataPath: compat, clientInstallPath: client, appId: appId, protonScript: proton, label: exe ?? p.basename(compat)));
  });
  return games;
}

const Set<String> _wineHelpers = {
  'steam.exe',
  'services.exe',
  'winedevice.exe',
  'plugplay.exe',
  'explorer.exe',
  'rpcss.exe',
  'svchost.exe',
  'tabtip.exe',
  'conhost.exe',
  'start.exe',
  'wineboot.exe',
  'winemenubuilder.exe',
  'steamwebhelper.exe',
};

/// Picks the running game for a trainer bound to [appId]. Matches the AppID
/// the launcher exported, or the prefix folder name (`compatdata/<appId>`).
RunningGame? matchGame(List<RunningGame> games, int appId) =>
    games.where((g) => g.appId == appId || p.basename(g.compatDataPath) == '$appId').firstOrNull;

/// Finds the Proton installation directory from a prefix's `config_info`.
///
/// `config_info` lines 2+ are paths *inside* the Proton install
/// (e.g. `.../GE-Proton9-20/files/share/fonts/`). Walk up from each path until a
/// directory containing a `proton` script is found. This handles Steam,
/// compatibilitytools.d, Heroic (`.../tools/proton/GE-Proton-latest/...`) and
/// paths where an unrelated parent folder happens to contain "proton".
String? protonDirFromConfigInfo(String content, bool Function(String path) isFile) {
  for (final raw in const LineSplitter().convert(content)) {
    final line = raw.trim();
    if (!line.startsWith('/')) continue;
    var dir = p.normalize(line);
    while (dir != '/' && dir.isNotEmpty && dir != '.') {
      if (isFile(p.join(dir, 'proton'))) return dir;
      dir = p.dirname(dir);
    }
  }
  return null;
}

/// Steam install locations, in lookup order. A non-empty [override] (set by the
/// user in Settings) is the only candidate.
List<String> steamRootCandidates(String home, {String? override}) {
  if (override != null && override.isNotEmpty) return [override];
  return [
    '$home/.steam/root',
    '$home/.local/share/Steam',
    '$home/.steam/steam',
    '$home/.steam/debian-installation',
    '$home/.var/app/com.valvesoftware.Steam/.local/share/Steam',
    '$home/snap/steam/common/.local/share/Steam',
  ];
}

/// First Steam root candidate that has a `steamapps` directory.
String? findSteamRoot(String home, {String? override, required bool Function(String path) dirExists}) {
  for (final candidate in steamRootCandidates(home, override: override)) {
    if (dirExists(p.join(candidate, 'steamapps'))) return candidate;
  }
  return null;
}

/// Chooses the Proton verb used to start a trainer next to a running game.
///
/// `run` goes through Proton's `steam.exe` helper. With GE-Proton 11 and Proton
/// Experimental 11 (tested without a running Steam client) that helper did not start a
/// second program while the game's own `steam.exe` ran in the same prefix: the trainer
/// never appeared.
/// `runinprefix` runs the exe with Wine directly and joins the game's wineserver,
/// also when the game runs inside the Steam Linux Runtime container. It needs an
/// already set-up prefix (callers ensure the game finished setting it up) and a
/// Proton build that knows the verb; older builds fall back to `run`.
String trainerVerb({required String protonScript}) => protonScript.contains('"runinprefix"') ? 'runinprefix' : 'run';
