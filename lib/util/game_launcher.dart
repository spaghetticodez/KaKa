import 'dart:async';
import 'dart:io';

import 'package:path/path.dart' as p;

import '../l10n/app_localizations.dart';
import 'dto.dart';
import 'proton.dart';

/// How long a freshly started trainer is watched for an immediate failure.
const Duration defaultStartupWatch = Duration(seconds: 4);
const int _keptLogs = 20;

class LaunchException implements Exception {
  final String message;

  /// The trainer's launch log, when one was written.
  final String? logPath;

  LaunchException(this.message, {this.logPath});

  @override
  String toString() => message;
}

/// Everything needed to run a trainer inside a game's Proton prefix.
class LaunchPlan {
  final String protonScript;
  final String compatDataPath;
  final String clientInstallPath;

  /// Proton verb, see [trainerVerb].
  final String verb;

  const LaunchPlan({required this.protonScript, required this.compatDataPath, required this.clientInstallPath, this.verb = 'run'});
}

bool _dirExists(String path) => Directory(path).existsSync();

bool _isFile(String path) => File(path).existsSync();

String _resolve(String path) {
  try {
    return Directory(path).resolveSymbolicLinksSync();
  } on FileSystemException {
    return path;
  }
}

String get _home => Platform.environment['HOME'] ?? '';

/// `$XDG_STATE_HOME/kaka/logs` (default `~/.local/state/kaka/logs`).
Directory defaultLogDir() {
  final state = Platform.environment['XDG_STATE_HOME'];
  final base = state != null && state.isNotEmpty ? state : p.join(_home, '.local', 'state');
  return Directory(p.join(base, 'kaka', 'logs'));
}

/// Finds the running game for a trainer and starts the trainer in its Proton prefix.
class TrainerLauncher {
  TrainerLauncher({this.steamPath = '', Directory? logDir, this.startupWatch = defaultStartupWatch}) : logDir = logDir ?? defaultLogDir();

  /// Steam install root chosen in Settings. Empty means auto-detect.
  String steamPath;

  /// Per-launch logs; the newest [_keptLogs] are kept.
  final Directory logDir;

  final Duration startupWatch;

  /// Trainers started in this session (entry id -> trainer path), stopped on exit.
  final Map<String, String> _started = {};

  /// Resolves the plan and starts the trainer; returns the plan that was used.
  /// Throws [LaunchException] with a user-facing message on failure.
  Future<LaunchPlan> launch(TrainerEntry entry, AppLocalizations l10n) async {
    final plan = resolvePlan(entry, l10n);
    await start(entry, plan, l10n);
    return plan;
  }

  /// Works out which prefix and Proton build to use. Throws [LaunchException]
  /// with a user-facing message when that is impossible.
  LaunchPlan resolvePlan(TrainerEntry entry, AppLocalizations l10n) {
    if (!_isFile(entry.trainerPath)) throw LaunchException(l10n.trainerPathNotFound(entry.trainerPath));

    final games = detectRunningGames(readProcesses());
    final appId = entry.steamAppId;
    // The game must already run. Starting a trainer first does not work with
    // Steam: its 'waitforexitandrun' waits for the prefix's wineserver (kept alive
    // by the trainer) to exit, so the game would hang until the trainer closes.
    final RunningGame game;
    if (appId != null) {
      game = matchGame(games, appId) ?? (throw LaunchException(l10n.gameNotRunning));
    } else if (games.length == 1) {
      game = games.single;
    } else if (games.length > 1) {
      throw LaunchException(l10n.multipleGamesRunning(games.map((g) => '• ${g.displayName}').join('\n')));
    } else {
      throw LaunchException(l10n.gameNotRunning);
    }

    // Prefer what the game's own launcher exported: always the right drive and Steam root.
    final prefix = game.compatDataPath;
    final client = game.clientInstallPath ?? _steamRoot() ?? '';

    var proton = game.protonScript;
    if (proton == null || !_isFile(proton)) {
      final configInfo = File(p.join(prefix, 'config_info'));
      final dir = configInfo.existsSync() ? protonDirFromConfigInfo(configInfo.readAsStringSync(), _isFile) : null;
      if (dir == null) throw LaunchException(l10n.protonPathNotFound(prefix));
      proton = p.join(dir, 'proton');
    }

    // Proton writes <compatdata>/version as the last step of prefix setup (under
    // pfx.lock) and deletes it before an upgrade; pfx/system.reg appears much
    // earlier. Without it the game is still setting up the prefix, and a second
    // Proton instance racing into it could break it.
    if (!_isFile(p.join(prefix, 'version'))) throw LaunchException(l10n.gameStarting);

    String script;
    try {
      script = File(proton).readAsStringSync();
    } on FileSystemException {
      script = '';
    }
    return LaunchPlan(protonScript: proton, compatDataPath: prefix, clientInstallPath: client, verb: trainerVerb(protonScript: script));
  }

  String? _steamRoot() {
    final root = findSteamRoot(_home, override: steamPath, dirExists: _dirExists);
    return root == null ? null : _resolve(root);
  }

  /// Starts the trainer through Proton and watches it for [startupWatch].
  /// Throws [LaunchException] with the log tail if it exits with an error in that time.
  Future<void> start(TrainerEntry entry, LaunchPlan plan, AppLocalizations l10n) async {
    final logFile = await _newLogFile(entry.name);
    final sink = logFile.openWrite();
    sink
      ..writeln('# KaKa trainer launch ${DateTime.now().toIso8601String()}')
      ..writeln('# trainer: ${entry.trainerPath}')
      ..writeln('# proton:  ${plan.protonScript} ${plan.verb}')
      ..writeln('# prefix:  ${plan.compatDataPath}')
      ..writeln('# steam:   ${plan.clientInstallPath}');

    final Process process;
    try {
      process = await Process.start(
        plan.protonScript,
        [plan.verb, entry.trainerPath],
        // Trainers often load files next to themselves.
        workingDirectory: p.dirname(entry.trainerPath),
        environment: {
          'STEAM_COMPAT_CLIENT_INSTALL_PATH': plan.clientInstallPath,
          'STEAM_COMPAT_DATA_PATH': plan.compatDataPath,
          trainerMarkerEnv: '1',
          trainerIdEnv: entry.id,
        },
      );
    } catch (e) {
      sink.writeln('# failed to start: $e');
      await sink.close();
      rethrow;
    }
    _started[entry.id] = entry.trainerPath;

    final stdoutDone = process.stdout.listen(sink.add).asFuture<void>();
    final stderrDone = process.stderr.listen(sink.add).asFuture<void>();
    final finished = Future.wait<Object?>([process.exitCode, stdoutDone, stderrDone]).then((results) async {
      final code = results[0] as int;
      sink.writeln('# exit code $code');
      await sink.close();
      return code;
    });

    final code = await Future.any<int?>([finished, Future<int?>.delayed(startupWatch, () => null)]);
    if (code != null && code != 0) {
      final lines = (await logFile.readAsLines()).where((l) => !l.startsWith('# ')).toList();
      final tail = lines.skip(lines.length > 15 ? lines.length - 15 : 0).join('\n');
      throw LaunchException(l10n.trainerExited(code, tail, logFile.path), logPath: logFile.path);
    }
  }

  Future<File> _newLogFile(String name) async {
    await logDir.create(recursive: true);

    final logs = logDir.listSync().whereType<File>().where((f) => f.path.endsWith('.log')).toList()..sort((a, b) => a.path.compareTo(b.path));
    for (final old in logs.take(logs.length >= _keptLogs ? logs.length - _keptLogs + 1 : 0)) {
      try {
        old.deleteSync();
      } on FileSystemException {
        // Best effort only.
      }
    }

    final stamp = DateTime.now().toIso8601String().split('.').first.replaceAll(':', '-');
    var safe = name.replaceAll(RegExp(r'[^A-Za-z0-9._-]+'), '_');
    if (safe.length > 40) safe = safe.substring(0, 40);
    return File(p.join(logDir.path, '$stamp-$safe.log'));
  }

  /// Ids of [entries] with a live trainer process (see [isTrainerProcess]).
  /// Also finds trainers a previous KaKa session left running.
  Set<String> runningIds(Iterable<TrainerEntry> entries, Iterable<ProcessInfo> processes) {
    final procs = processes.where((proc) => proc.env[trainerMarkerEnv] == '1').toList();
    return {
      for (final e in entries)
        if (procs.any((proc) => isTrainerProcess(proc, e.id, e.trainerPath))) e.id,
    };
  }

  /// Sends SIGTERM to the processes of [entry]'s trainer; returns how many.
  int stop(TrainerEntry entry, Iterable<ProcessInfo> processes) {
    var signalled = 0;
    for (final proc in processes) {
      if (isTrainerProcess(proc, entry.id, entry.trainerPath) && Process.killPid(proc.pid, ProcessSignal.sigterm)) signalled++;
    }
    return signalled;
  }

  /// Stops trainers started by this launcher. Only processes carrying the KaKa
  /// marker, the entry id *and* the trainer's executable name are signalled, so
  /// Wine services shared with a running game (wineserver, explorer.exe, ...) survive.
  Future<void> stopAll() async {
    if (_started.isEmpty) return;
    for (final proc in readProcesses()) {
      if (proc.env[trainerMarkerEnv] != '1') continue;
      final path = _started[proc.env[trainerIdEnv]];
      if (path != null && isTrainerProcess(proc, proc.env[trainerIdEnv]!, path)) Process.killPid(proc.pid, ProcessSignal.sigterm);
    }
  }
}
