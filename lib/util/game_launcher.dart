import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path/path.dart' as p;

import '../l10n/app_localizations.dart';
import 'dto.dart';
import 'proton.dart';
import 'widget.dart';

/// Steam install root chosen in Settings. Empty means auto-detect.
String customSteamPath = '';

/// How long a freshly started trainer is watched for an immediate failure.
const Duration _startupWatch = Duration(seconds: 4);
const int _keptLogs = 20;

/// Executable names of trainers started in this session (lower case), used to
/// stop exactly those processes on exit.
final Set<String> _startedTrainerExes = {};

class LaunchException implements Exception {
  final String message;

  LaunchException(this.message);

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

/// Launches [entry] and reports success or failure in the UI.
Future<void> launchTrainer(BuildContext context, TrainerEntry entry) async {
  final l10n = AppLocalizations.of(context)!;
  try {
    final plan = resolveLaunchPlan(entry, l10n);
    await startTrainer(entry, plan, l10n);
    if (context.mounted) showSnakeBar(context: context, message: l10n.launched(entry.name), time: 2);
  } catch (e) {
    debugPrint('Launch failed: $e');
    if (context.mounted) _showError(context, l10n, e.toString());
  }
}

/// Works out which prefix and Proton build to use. Throws [LaunchException]
/// with a user-facing message when that is impossible.
LaunchPlan resolveLaunchPlan(TrainerEntry entry, AppLocalizations l10n) {
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
  final root = findSteamRoot(_home, override: customSteamPath, dirExists: _dirExists);
  return root == null ? null : _resolve(root);
}

/// Starts the trainer through Proton and watches it for [watch]. Throws
/// [LaunchException] with the log tail if it exits with an error in that time.
Future<void> startTrainer(TrainerEntry entry, LaunchPlan plan, AppLocalizations l10n, {Directory? logDir, Duration watch = _startupWatch}) async {
  final logFile = await _newLogFile(entry.name, logDir ?? _defaultLogDir());
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
      },
    );
  } catch (e) {
    sink.writeln('# failed to start: $e');
    await sink.close();
    rethrow;
  }
  _startedTrainerExes.add(p.basename(entry.trainerPath).toLowerCase());

  final stdoutDone = process.stdout.listen(sink.add).asFuture<void>();
  final stderrDone = process.stderr.listen(sink.add).asFuture<void>();
  final finished = Future.wait<Object?>([process.exitCode, stdoutDone, stderrDone]).then((results) async {
    final code = results[0] as int;
    sink.writeln('# exit code $code');
    await sink.close();
    return code;
  });

  final code = await Future.any<int?>([finished, Future<int?>.delayed(watch, () => null)]);
  if (code != null && code != 0) {
    final lines = (await logFile.readAsLines()).where((l) => !l.startsWith('# ')).toList();
    final tail = lines.skip(lines.length > 15 ? lines.length - 15 : 0).join('\n');
    throw LaunchException(l10n.trainerExited(code, tail, logFile.path));
  }
}

Directory _defaultLogDir() {
  final state = Platform.environment['XDG_STATE_HOME'];
  final base = state != null && state.isNotEmpty ? state : p.join(_home, '.local', 'state');
  return Directory(p.join(base, 'kaka', 'logs'));
}

Future<File> _newLogFile(String name, Directory dir) async {
  await dir.create(recursive: true);

  final logs = dir.listSync().whereType<File>().where((f) => f.path.endsWith('.log')).toList()..sort((a, b) => a.path.compareTo(b.path));
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
  return File(p.join(dir.path, '$stamp-$safe.log'));
}

/// Stops trainers started by this KaKa session. Only processes carrying the
/// KaKa marker *and* the trainer's executable name are signalled, so Wine
/// services shared with a running game (wineserver, explorer.exe, ...) survive.
Future<void> stopAllTrainers() async {
  if (_startedTrainerExes.isEmpty) return;
  for (final proc in readProcesses()) {
    if (proc.env[trainerMarkerEnv] != '1') continue;
    final mentionsTrainer = proc.cmdline.any((arg) => _startedTrainerExes.contains(p.basename(arg.replaceAll('\\', '/')).toLowerCase()));
    if (!mentionsTrainer) continue;
    Process.killPid(proc.pid, ProcessSignal.sigterm);
  }
}

void _showError(BuildContext context, AppLocalizations l10n, String message) {
  showDialog<void>(
    context: context,
    builder: (BuildContext context) {
      return AlertDialog(
        title: Text(l10n.launchFail),
        content: SingleChildScrollView(child: SelectableText(message)),
        actions: <Widget>[
          TextButton(
            child: Text(l10n.ok),
            onPressed: () => Navigator.of(context).pop(),
          ),
        ],
      );
    },
  );
}
