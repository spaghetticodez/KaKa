import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/l10n/app_localizations.dart';
import 'package:kaka/state/library_controller.dart';
import 'package:kaka/util/game_launcher.dart';
import 'package:kaka/util/proton.dart';
import 'package:kaka/util/storage.dart';
import 'package:kaka/util/trainer_data.dart';
import 'package:path/path.dart' as p;

import 'fakes.dart';

void main() {
  final l10n = lookupAppLocalizations(const Locale('en'));
  late MemoryStorage storage;
  late FakeLauncher launcher;
  late LibraryController controller;
  late CountingScan scan;

  setUp(() async {
    storage = MemoryStorage(library: [trainer('a'), trainer('b')], config: const AppConfig(language: 'German', steamPath: '/steam'));
    launcher = FakeLauncher();
    scan = CountingScan();
    controller = LibraryController(storage: storage, launcher: launcher, processes: scan.call, orphanPollInterval: const Duration(milliseconds: 20));
    await controller.load();
  });

  test('load reads the library and settings and hands the Steam path to the launcher', () {
    expect(controller.loading, isFalse);
    expect(controller.trainers.map((e) => e.id), ['a', 'b']);
    expect(controller.language, 'German');
    expect(launcher.steamPath, '/steam');
  });

  test('an unknown saved language falls back to English', () async {
    final c = LibraryController(storage: MemoryStorage(config: const AppConfig(language: 'Klingon')), launcher: FakeLauncher());
    await c.load();
    expect(c.language, 'English');
  });

  test('add, update and remove are saved immediately', () async {
    await controller.add(trainer('c'));
    expect(storage.library.map((e) => e.id), ['c', 'a', 'b']);

    await controller.update(trainer('a')..name = 'Renamed');
    expect(storage.library.firstWhere((e) => e.id == 'a').name, 'Renamed');

    await controller.remove('b');
    expect(storage.library.map((e) => e.id), ['c', 'a']);
    expect(storage.libraryWrites, 3);
  });

  test('the trainer list cannot be changed behind the controller', () {
    expect(() => controller.trainers.add(trainer('x')), throwsUnsupportedError);
  });

  test('launch moves the entry to the front and returns null on success', () async {
    expect(await controller.launch(controller.trainers.last, l10n), isNull);
    expect(launcher.launched, ['b']);
    expect(storage.library.map((e) => e.id), ['b', 'a']);
  });

  test('a successful launch records the prefix; a failed one keeps the old value', () async {
    await controller.launch(controller.trainers.first, l10n);
    expect(storage.library.first.lastPrefix, '/compat/1245620');

    launcher
      ..prefix = '/compat/other'
      ..failure = LaunchException('no');
    await controller.launch(controller.trainers.first, l10n);
    expect(storage.library.first.lastPrefix, '/compat/1245620');
  });

  test('resetTrainerData refuses while something runs in the prefix, then cleans it', () async {
    final tmp = Directory.systemTemp.createTempSync('kaka_ctl_reset_');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final compat = p.join(tmp.path, 'compatdata', '7');
    final fling = Directory(p.join(compat, 'pfx', 'drive_c', 'users', 'steamuser', 'AppData', 'Local', 'FLiNGTrainer'))..createSync(recursive: true);
    final running = CountingScan([
      ProcessInfo(1, {'STEAM_COMPAT_DATA_PATH': compat}, ['game.exe']),
    ]);
    final c = LibraryController(storage: MemoryStorage(), launcher: FakeLauncher(), processes: running.call);
    final entry = trainer('x', lastPrefix: compat);

    await expectLater(c.resetTrainerData(entry), throwsA(isA<CleanupRefused>()));
    expect(fling.existsSync(), isTrue);

    running.processes = [];
    expect(await c.resetTrainerData(entry), 1);
    expect(fling.existsSync(), isFalse);
    expect(await c.resetTrainerData(trainer('never-launched')), 0);
  });

  test('a running trainer is not launched twice; stop clears it', () async {
    final entry = controller.trainers.first;
    expect(await controller.launch(entry, l10n), isNull);
    expect(controller.isRunning(entry.id), isTrue);

    final again = await controller.launch(entry, l10n);
    expect(again?.message, l10n.trainerAlreadyRunning(entry.name));
    expect(launcher.launched, [entry.id], reason: 'no second process');

    expect(await controller.stop(entry), isTrue);
    expect(controller.isRunning(entry.id), isFalse);
  });

  test('the launch guard also catches a trainer only a scan can see', () async {
    launcher.orphanProcs.add('a');
    final again = await controller.launch(controller.trainers.first, l10n);
    expect(again?.message, l10n.trainerAlreadyRunning('Game a'));
    expect(launcher.launched, isEmpty);
  });

  test('stop gives up after the timeout when the trainer does not exit', () async {
    launcher
      ..live.add('a')
      ..stubborn = true;
    expect(await controller.stop(controller.trainers.first, timeout: const Duration(milliseconds: 300)), isFalse);
    expect(controller.isRunning('a'), isTrue);
  });

  test('idle: one scan at startup, then none while no orphan trainer exists', () async {
    await controller.startMonitoring();
    expect(scan.calls, 1);
    await Future<void>.delayed(const Duration(milliseconds: 150));
    expect(scan.calls, 1, reason: 'no periodic /proc scans when nothing needs them');

    // A trainer this session starts and its exit are events, not scans.
    var notified = 0;
    controller.addListener(() => notified++);
    launcher
      ..live.add('b')
      ..emit();
    expect(controller.isRunning('b'), isTrue);
    launcher
      ..live.remove('b')
      ..emit();
    expect(controller.isRunning('b'), isFalse);
    expect(notified, 2);
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(scan.calls, 1);
  });

  test('a trainer left by an earlier session is re-checked only until it is gone', () async {
    launcher.orphanProcs.add('b');
    await controller.startMonitoring();
    expect(controller.isRunning('b'), isTrue);

    launcher.orphanProcs.clear();
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(controller.isRunning('b'), isFalse);
    final callsWhenGone = scan.calls;
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(scan.calls, callsWhenGone, reason: 'polling stops with the last orphan');
  });

  test('dispose stops the orphan re-check and ignores launcher events', () async {
    launcher.orphanProcs.add('b');
    await controller.startMonitoring();
    controller.dispose();
    final calls = scan.calls;
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(scan.calls, calls);
    launcher.emit(); // must not throw "used after being disposed"
  });

  test('launch returns the LaunchException, with its log path', () async {
    launcher.failure = LaunchException(l10n.gameNotRunning, logPath: '/logs/x.log');
    final error = await controller.launch(controller.trainers.first, l10n);
    expect(error?.message, l10n.gameNotRunning);
    expect(error?.logPath, '/logs/x.log');
  });

  test('launch also reports unexpected errors instead of throwing', () async {
    launcher.failure = StateError('boom');
    expect((await controller.launch(controller.trainers.first, l10n))?.message, contains('boom'));
  });

  test('logs open as local file URIs; the folder is created first; opener errors are swallowed', () async {
    final tmp = Directory.systemTemp.createTempSync('kaka_ctl_');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final opened = <Uri>[];
    final logDir = Directory(p.join(tmp.path, 'state', 'kaka', 'logs'));
    final c = LibraryController(
        storage: MemoryStorage(),
        launcher: FakeLauncher(logDir: logDir),
        openUri: (uri) async {
          opened.add(uri);
          return true;
        });

    expect(await c.openLogFolder(), isTrue);
    expect(logDir.existsSync(), isTrue);
    expect(await c.openLog('/logs/a b.log'), isTrue);
    expect(opened, [Uri.directory(logDir.path), Uri.file('/logs/a b.log')]);
    expect(opened.every((u) => u.scheme == 'file'), isTrue);

    final failing =
        LibraryController(storage: MemoryStorage(), launcher: FakeLauncher(logDir: logDir), openUri: (_) => throw StateError('no handler'));
    expect(await failing.openLog('/x.log'), isFalse);
  });

  test('settings are saved; unknown languages are rejected', () async {
    await controller.setLanguage('中文');
    await controller.setSteamPath('');
    expect(storage.config.language, '中文');
    expect(storage.config.steamPath, '');
    expect(launcher.steamPath, '');

    await controller.setLanguage('Klingon');
    expect(controller.language, '中文');
  });

  test('shutdown saves everything and stops this session\'s trainers', () async {
    await controller.shutdown();
    expect(storage.libraryWrites, 1);
    expect(storage.configWrites, 1);
    expect(launcher.stopAllCalls, 1);
  });

  test('listeners are notified on every change', () async {
    var notified = 0;
    controller.addListener(() => notified++);
    await controller.add(trainer('c'));
    await controller.remove('c');
    await controller.setLanguage('English');
    expect(notified, 3);
  });
}
