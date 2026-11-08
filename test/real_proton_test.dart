// Opt-in tests against a REAL Proton build. Skipped unless KAKA_REAL_PROTON
// points at a `proton` script. Wine's own cmd.exe stands in for games and
// trainers. Optionally set KAKA_SLR to a SteamLinuxRuntime_* directory to run
// the fake game inside the Steam Linux Runtime container, like Steam does.
//
//   KAKA_REAL_PROTON="$HOME/.local/share/Steam/steamapps/common/Proton - Experimental/proton" \
//   KAKA_STEAM_ROOT="$HOME/.local/share/Steam" \
//   KAKA_SLR="$HOME/.local/share/Steam/steamapps/common/SteamLinuxRuntime_4" \
//   flutter test test/real_proton_test.dart
library;

import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/l10n/app_localizations.dart';
import 'package:kaka/util/dto.dart';
import 'package:kaka/util/game_launcher.dart';
import 'package:kaka/util/proton.dart';
import 'package:path/path.dart' as p;

final String proton = Platform.environment['KAKA_REAL_PROTON'] ?? '';
final String steamRoot = Platform.environment['KAKA_STEAM_ROOT'] ?? '';
final String slr = Platform.environment['KAKA_SLR'] ?? '';

File copyCmd(Directory dir, String name) {
  dir.createSync(recursive: true);
  return File(p.join(p.dirname(proton), 'files', 'lib', 'wine', 'x86_64-windows', 'cmd.exe')).copySync(p.join(dir.path, name));
}

List<ProcessInfo> inPrefix(String prefix) => readProcesses().where((proc) => proc.env['STEAM_COMPAT_DATA_PATH'] == prefix).toList();

/// A Wine process (argv[0] is the Windows path of [exe]) running in [prefix].
bool wineRunning(String prefix, String exe, {bool? marked}) => inPrefix(prefix).any((proc) =>
    (marked == null || (proc.env[trainerMarkerEnv] == '1') == marked) &&
    proc.cmdline.isNotEmpty &&
    RegExp(r'^([A-Za-z]:\\|\\\\)').hasMatch(proc.cmdline.first) &&
    proc.cmdline.first.toLowerCase().endsWith(exe.toLowerCase()));

Future<bool> waitFor(bool Function() condition, Duration timeout) async {
  final deadline = DateTime.now().add(timeout);
  while (!condition() && DateTime.now().isBefore(deadline)) {
    await Future<void>.delayed(const Duration(milliseconds: 500));
  }
  return condition();
}

String describe(String prefix) =>
    inPrefix(prefix).map((e) => '  ${e.pid}${e.env[trainerMarkerEnv] == '1' ? ' [kaka]' : ''}: ${e.cmdline.join(' ')}').join('\n');

Future<void> killPrefix(String prefix) async {
  for (final proc in inPrefix(prefix)) {
    Process.killPid(proc.pid, ProcessSignal.sigterm);
  }
  if (!await waitFor(() => inPrefix(prefix).isEmpty, const Duration(seconds: 15))) {
    for (final proc in inPrefix(prefix)) {
      Process.killPid(proc.pid, ProcessSignal.sigkill);
    }
    await waitFor(() => inPrefix(prefix).isEmpty, const Duration(seconds: 5));
  }
}

void main() {
  final skip = proton.isEmpty ? 'set KAKA_REAL_PROTON to run' : null;
  final l10n = lookupAppLocalizations(const Locale('en'));

  test('fresh prefix: "run" creates it, the trainer runs, and KaKa can stop it', () async {
    final tmp = Directory.systemTemp.createTempSync('kaka_real_');
    final prefix = p.join(tmp.path, 'compatdata', '424242');
    addTearDown(() async {
      await killPrefix(prefix);
      tmp.deleteSync(recursive: true);
    });
    Directory(prefix).createSync(recursive: true);
    final trainer = copyCmd(Directory(p.join(tmp.path, 'trainers')), 'cmd.exe');
    final entry = TrainerEntry(id: 'real', name: 'Real Proton', trainerPath: trainer.path);
    final logs = Directory(p.join(tmp.path, 'logs'));
    final launcher = TrainerLauncher(logDir: logs);

    await launcher.start(entry, LaunchPlan(protonScript: proton, compatDataPath: prefix, clientInstallPath: steamRoot), l10n);
    final up = await waitFor(() => wineRunning(prefix, 'cmd.exe', marked: true), const Duration(minutes: 3));
    expect(up, isTrue, reason: 'log:\n${logs.listSync().whereType<File>().single.readAsStringSync()}');
    expect(File(p.join(prefix, 'pfx', 'system.reg')).existsSync(), isTrue);

    await launcher.stopAll();
    expect(await waitFor(() => !wineRunning(prefix, 'cmd.exe'), const Duration(seconds: 20)), isTrue);
  }, skip: skip, timeout: const Timeout(Duration(minutes: 5)));

  test('running game${slr.isEmpty ? '' : ' (Steam Linux Runtime)'}: KaKa detects it, joins its prefix and stops only the trainer', () async {
    final tmp = Directory.systemTemp.createTempSync('kaka_real_game_');
    final prefix = p.join(tmp.path, 'compatdata', '434343');
    Directory(prefix).createSync(recursive: true);
    final game = copyCmd(Directory(p.join(tmp.path, 'game')), 'FakeGame.exe');
    final trainer = copyCmd(Directory(p.join(tmp.path, 'trainers')), 'FakeTrainer.exe');

    final gameArgs = [
      if (slr.isNotEmpty) ...[p.join(slr, '_v2-entry-point'), '--verb=waitforexitandrun', '--'],
      proton,
      'waitforexitandrun',
      game.path
    ];
    final gameProcess = await Process.start(gameArgs.first, gameArgs.sublist(1), environment: {
      'STEAM_COMPAT_DATA_PATH': prefix,
      'STEAM_COMPAT_CLIENT_INSTALL_PATH': steamRoot,
      'SteamAppId': '434343',
      if (slr.isNotEmpty) 'STEAM_COMPAT_TOOL_PATHS': '${p.dirname(proton)}:$slr',
    });
    gameProcess.stdout.drain<void>();
    gameProcess.stderr.drain<void>();
    addTearDown(() async {
      gameProcess.kill();
      await killPrefix(prefix);
      tmp.deleteSync(recursive: true);
    });
    expect(await waitFor(() => wineRunning(prefix, 'FakeGame.exe'), const Duration(minutes: 3)), isTrue, reason: 'game did not start');

    final entry = TrainerEntry(id: 'game', name: 'Fake Trainer', trainerPath: trainer.path, steamAppId: 434343);
    final logs = Directory(p.join(tmp.path, 'logs'));
    final launcher = TrainerLauncher(logDir: logs);
    final plan = launcher.resolvePlan(entry, l10n);
    expect(plan.compatDataPath, prefix);
    expect(plan.verb, 'runinprefix',
        reason: 'proton=${plan.protonScript} exists=${File(plan.protonScript).existsSync()} '
            'system.reg=${File(p.join(prefix, 'pfx', 'system.reg')).existsSync()} pfx=${Directory(p.join(prefix, 'pfx')).existsSync() ? Directory(p.join(prefix, 'pfx')).listSync().map((e) => p.basename(e.path)).toList() : 'missing'}\n${describe(prefix)}');

    await launcher.start(entry, plan, l10n);
    final up = await waitFor(() => wineRunning(prefix, 'FakeTrainer.exe', marked: true), const Duration(minutes: 1));
    // ignore: avoid_print
    print('processes in the prefix:\n${describe(prefix)}');
    expect(up, isTrue, reason: 'log:\n${logs.listSync().whereType<File>().single.readAsStringSync()}');
    expect(inPrefix(prefix).where((e) => e.cmdline.any((a) => a.endsWith('/wineserver'))), hasLength(1),
        reason: 'the trainer must join the game\'s wineserver, not start a second one');

    // The Wine process of the trainer itself must carry the entry id, so KaKa
    // can show it as running and stop exactly this trainer.
    expect(
        inPrefix(prefix).any((proc) => proc.env[trainerIdEnv] == entry.id && proc.cmdline.any((a) => a.toLowerCase().endsWith('faketrainer.exe') && a.contains(r'\'))),
        isTrue,
        reason: describe(prefix));
    expect(launcher.runningIds([entry], readProcesses()), {entry.id});

    expect(launcher.stop(entry, readProcesses()), greaterThan(0));
    expect(await waitFor(() => !wineRunning(prefix, 'FakeTrainer.exe'), const Duration(seconds: 20)), isTrue);
    expect(launcher.runningIds([entry], readProcesses()), isEmpty);
    expect(wineRunning(prefix, 'FakeGame.exe'), isTrue, reason: 'stopping trainers must not touch the game');
  }, skip: skip, timeout: const Timeout(Duration(minutes: 6)));
}
