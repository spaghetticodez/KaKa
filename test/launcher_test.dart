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

  setUp(() async {
    tmp = Directory.systemTemp.createTempSync('kaka_launch_');
    protonDir = p.join(tmp.path, 'compatibilitytools.d', 'Fake Proton');
    Directory(p.join(protonDir, 'files', 'share', 'fonts')).createSync(recursive: true);
    final script = File(p.join(protonDir, 'proton'))..writeAsStringSync(fakeProton);
    await Process.run('chmod', ['+x', script.path]);

    prefix = p.join(tmp.path, 'library', 'steamapps', 'compatdata', '$fakeAppId');
    Directory(p.join(prefix, 'pfx')).createSync(recursive: true);
    File(p.join(prefix, 'config_info')).writeAsStringSync('fake-1\n$protonDir/files/share/fonts/\n$protonDir/files/lib/\n');

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
    final plan = resolveLaunchPlan(entry('Trainer.exe'), l10n);
    expect(plan.compatDataPath, prefix);
    expect(plan.protonScript, p.join(protonDir, 'proton'));
  });

  test('starts the trainer inside the prefix with the marker, then stops it on exit', () async {
    final e = entry('Trainer.exe');
    await startTrainer(e, resolveLaunchPlan(e, l10n), l10n, logDir: Directory(p.join(tmp.path, 'logs')), watch: const Duration(milliseconds: 800));

    final record = File(p.join(protonDir, 'record-Trainer.exe.txt')).readAsLinesSync();
    expect(record, contains('ARGS:run ${e.trainerPath}'));
    expect(record, contains('PWD:${trainers.path}'));
    expect(record, contains('DATA:$prefix'));
    expect(record, contains('MARK:1'));

    bool trainerAlive() => readProcesses().any((proc) => proc.env[trainerMarkerEnv] == '1' && proc.cmdline.any((a) => a.endsWith('Trainer.exe')));
    expect(trainerAlive(), isTrue);
    expect(detectRunningGames(readProcesses()).where((g) => g.compatDataPath == prefix), hasLength(1),
        reason: 'the trainer itself must not show up as a second game');

    await stopAllTrainers();
    for (var i = 0; i < 50 && trainerAlive(); i++) {
      await Future<void>.delayed(const Duration(milliseconds: 20));
    }
    expect(trainerAlive(), isFalse);
    expect(game.kill(ProcessSignal.sigcont), isTrue, reason: 'the game process must survive stopAllTrainers');

    final log = Directory(p.join(tmp.path, 'logs')).listSync().single as File;
    expect(log.readAsStringSync(), contains('hello from fake proton'));
  });

  test('a trainer that exits immediately reports the exit code and output', () async {
    final e = entry('Crash.exe');
    await expectLater(
      startTrainer(e, resolveLaunchPlan(e, l10n), l10n, logDir: Directory(p.join(tmp.path, 'logs'))),
      throwsA(isA<LaunchException>().having((x) => x.message, 'message', allOf(contains('exit code 3'), contains('err: boom')))),
    );
  });

  test('missing trainer file and an unknown AppID fail with a clear error', () {
    expect(() => resolveLaunchPlan(entry('Missing.exe'), l10n), throwsA(isA<LaunchException>()));
    expect(() => resolveLaunchPlan(entry('Trainer.exe', appId: 1), l10n), throwsA(isA<LaunchException>()));
  });
}
