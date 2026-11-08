// End-to-end launcher test against the real /proc: a fake game process that
// exports a Proton environment, and a fake `proton` script that records what
// KaKa passes to it.
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/l10n/app_localizations.dart';
import 'package:kaka/util/dto.dart';
import 'package:kaka/util/game_launcher.dart';
import 'package:kaka/util/proton.dart';
import 'package:path/path.dart' as p;

const int fakeAppId = 987654321;

const String fakeProton = r'''#!/bin/bash
# Like real Proton: case "$1" in run|runinprefix) ... (KaKa checks for "runinprefix")
{
  echo "ARGS:$*"
  echo "PWD:$PWD"
  echo "DATA:$STEAM_COMPAT_DATA_PATH"
  echo "MARK:$KAKA_TRAINER"
} > "$(dirname "$0")/record-$(basename "$2").txt"
echo "wine: hello from fake proton"
if [ "$(basename "$2")" = "Crash.exe" ]; then
  echo "err: boom" >&2
  exit 3
fi
sleep 60 &
trap 'kill $!; exit 0' TERM
wait
''';

void main() {
  final l10n = lookupAppLocalizations(const Locale('en'));
  late Directory tmp;
  late String prefix;
  late String protonDir;
  late Directory trainers;
  late Process game;
  late TrainerLauncher launcher;

  setUp(() async {
    tmp = Directory.systemTemp.createTempSync('kaka_launch_');
    protonDir = p.join(tmp.path, 'compatibilitytools.d', 'Fake Proton');
    Directory(p.join(protonDir, 'files', 'share', 'fonts')).createSync(recursive: true);
    final script = File(p.join(protonDir, 'proton'))..writeAsStringSync(fakeProton);
    await Process.run('chmod', ['+x', script.path]);

    prefix = p.join(tmp.path, 'library', 'steamapps', 'compatdata', '$fakeAppId');
    Directory(p.join(prefix, 'pfx')).createSync(recursive: true);
    File(p.join(prefix, 'pfx', 'system.reg')).writeAsStringSync('WINE REGISTRY Version 2\n');
    // Proton writes <compatdata>/version last, when the prefix is fully set up.
    File(p.join(prefix, 'version')).writeAsStringSync('11.0-100\n');
    File(p.join(prefix, 'config_info')).writeAsStringSync('fake-1\n$protonDir/files/share/fonts/\n$protonDir/files/lib/\n');

    launcher = TrainerLauncher(logDir: Directory(p.join(tmp.path, 'logs')), startupWatch: const Duration(milliseconds: 800));
    trainers = Directory(p.join(tmp.path, 'my trainers'))..createSync();
    File(p.join(trainers.path, 'Trainer.exe')).writeAsStringSync('MZ');
    File(p.join(trainers.path, 'Crash.exe')).writeAsStringSync('MZ');

    game = await Process.start('bash', ['-c', 'exec -a FakeGame.exe sleep 60'],
        environment: {'STEAM_COMPAT_DATA_PATH': prefix, 'SteamAppId': '$fakeAppId'});
    // Wait until the exec has replaced bash, so /proc shows the game's cmdline.
    for (var i = 0; i < 50; i++) {
      if (detectRunningGames(readProcesses()).any((g) => g.appId == fakeAppId)) break;
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
  });

  tearDown(() {
    game.kill();
    tmp.deleteSync(recursive: true);
  });

  TrainerEntry entry(String exe, {int? appId = fakeAppId}) =>
      TrainerEntry(id: exe, name: exe, trainerPath: p.join(trainers.path, exe), steamAppId: appId);

  test('resolves the running game prefix and the Proton build from config_info', () {
    final plan = launcher.resolvePlan(entry('Trainer.exe'), l10n);
    expect(plan.compatDataPath, prefix);
    expect(plan.protonScript, p.join(protonDir, 'proton'));
    expect(plan.verb, 'runinprefix');
  });

  test('refuses while the game is still setting up its prefix (system.reg present, version missing)', () {
    File(p.join(prefix, 'version')).deleteSync();
    expect(File(p.join(prefix, 'pfx', 'system.reg')).existsSync(), isTrue);
    expect(() => launcher.resolvePlan(entry('Trainer.exe'), l10n),
        throwsA(isA<LaunchException>().having((x) => x.message, 'message', l10n.gameStarting)));
  });

  test('falls back to "run" for a Proton build without runinprefix', () {
    File(p.join(protonDir, 'proton')).writeAsStringSync(fakeProton.replaceAll('runinprefix', 'oldverb'));
    expect(launcher.resolvePlan(entry('Trainer.exe'), l10n).verb, 'run');
  });

  test('starts the trainer inside the prefix with the marker, then stops it on exit', () async {
    final e = entry('Trainer.exe');
    await launcher.launch(e, l10n);

    final record = File(p.join(protonDir, 'record-Trainer.exe.txt')).readAsLinesSync();
    expect(record, contains('ARGS:runinprefix ${e.trainerPath}'));
    expect(record, contains('PWD:${trainers.path}'));
    expect(record, contains('DATA:$prefix'));
    expect(record, contains('MARK:1'));

    bool trainerAlive() => readProcesses().any((proc) => proc.env[trainerMarkerEnv] == '1' && proc.cmdline.any((a) => a.endsWith('Trainer.exe')));
    expect(trainerAlive(), isTrue);
    expect(detectRunningGames(readProcesses()).where((g) => g.compatDataPath == prefix), hasLength(1),
        reason: 'the trainer itself must not show up as a second game');

    await launcher.stopAll();
    for (var i = 0; i < 50 && trainerAlive(); i++) {
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
    expect(trainerAlive(), isFalse);
    expect(game.kill(ProcessSignal.sigcont), isTrue, reason: 'the game process must survive stopAll');

    final log = Directory(p.join(tmp.path, 'logs')).listSync().single as File;
    expect(log.readAsStringSync(), contains('hello from fake proton'));
  });

  test('a trainer that exits immediately reports the exit code and output', () async {
    final e = entry('Crash.exe');
    await expectLater(
      launcher.launch(e, l10n),
      throwsA(isA<LaunchException>()
          .having((x) => x.message, 'message', allOf(contains('exit code 3'), contains('err: boom')))
          .having((x) => x.logPath != null && File(x.logPath!).existsSync(), 'log file exists', isTrue)),
    );
  });

  test('missing trainer file fails clearly; a bound game that is not running is never started into', () {
    expect(() => launcher.resolvePlan(entry('Missing.exe'), l10n), throwsA(isA<LaunchException>()));
    // Starting a trainer before the game would make Steam's waitforexitandrun hang the game.
    expect(() => launcher.resolvePlan(entry('Trainer.exe', appId: 1), l10n),
        throwsA(isA<LaunchException>().having((x) => x.message, 'message', l10n.gameNotRunning)));
  });
}
