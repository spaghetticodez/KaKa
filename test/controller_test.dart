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

  setUp(() async {
    storage = MemoryStorage(library: [trainer('a'), trainer('b')], config: const AppConfig(language: 'German', steamPath: '/steam'));
    launcher = FakeLauncher();
    controller = LibraryController(storage: storage, launcher: launcher);
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

  test('resetTrainerData refuses while something runs in the prefix, then cleans it', () {
    final tmp = Directory.systemTemp.createTempSync('kaka_ctl_reset_');
    addTearDown(() => tmp.deleteSync(recursive: true));
    final compat = p.join(tmp.path, 'compatdata', '7');
    final fling = Directory(p.join(compat, 'pfx', 'drive_c', 'users', 'steamuser', 'AppData', 'Local', 'FLiNGTrainer'))..createSync(recursive: true);
    var running = <ProcessInfo>[
      ProcessInfo(1, {'STEAM_COMPAT_DATA_PATH': compat}, ['game.exe']),
    ];
    final c = LibraryController(storage: MemoryStorage(), launcher: FakeLauncher(), processes: () => running);
    final entry = trainer('x', lastPrefix: compat);

    expect(() => c.resetTrainerData(entry), throwsA(isA<CleanupRefused>()));
    expect(fling.existsSync(), isTrue);

    running = [];
    expect(c.resetTrainerData(entry), 1);
    expect(fling.existsSync(), isFalse);
    expect(c.resetTrainerData(trainer('never-launched')), 0);
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
