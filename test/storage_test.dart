import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/util/dto.dart';
import 'package:kaka/util/storage.dart';
import 'package:path/path.dart' as p;

void main() {
  group('TrainerEntry', () {
    test('survives a JSON round trip', () {
      final entry = TrainerEntry(id: 'abc', name: 'Test Game', trainerPath: '/t/trainer.exe', coverImagePath: '/t/cover.png', steamAppId: 1245620);
      final restored = TrainerEntry.fromJson(jsonDecode(jsonEncode(entry.toJson())) as Map<String, dynamic>)!;
      expect(restored.toJson(), entry.toJson());
    });

    test('imports a LaLa custom game and rejects LaLa online games', () {
      final custom = TrainerEntry.fromJson({'name': 'Old', 'app_id': 1999999999, 'cover_image_path': '', 'trainer_path': '/t/old.exe'})!;
      expect(custom.id, 'lala-1999999999');
      expect(custom.steamAppId, isNull, reason: 'LaLa app_id of custom games was random, not a Steam AppID');
      expect(TrainerEntry.fromJson({'name': 'Online', 'app_id': 1245620, 'cover_image_url': 'https://x', 'trainers': []}), isNull);
    });

    test('newId is random hex', () {
      expect(TrainerEntry.newId(), matches(RegExp(r'^[0-9a-f]{16}$')));
      expect(TrainerEntry.newId(), isNot(TrainerEntry.newId()));
    });
  });

  group('LocalStorage', () {
    late Directory tmp;
    late Directory data;
    late Directory legacy;

    LocalStorage storage() => LocalStorage(dataDir: () async => data, legacyDir: legacy.path);

    setUp(() {
      tmp = Directory.systemTemp.createTempSync('kaka_storage_');
      data = Directory(p.join(tmp.path, 'data', 'io.github.spaghetticodez.kaka'));
      legacy = Directory(p.join(tmp.path, 'cache', 'com.aironheart.lala', 'config'))..createSync(recursive: true);
    });
    tearDown(() => tmp.deleteSync(recursive: true));

    test('first start imports LaLa custom trainers and config once', () async {
      File(p.join(legacy.path, 'library.json')).writeAsStringSync(jsonEncode([
        {'name': 'Online', 'app_id': 1, 'cover_image_url': 'https://x', 'trainers': []},
        {'name': 'Mine', 'app_id': 2000000000, 'cover_image_path': '', 'trainer_path': '/t/mine.exe'},
      ]));
      File(p.join(legacy.path, 'config.json')).writeAsStringSync(jsonEncode({'language': 'German', 'custom_steam_path': '/s'}));

      final s = storage();
      final entries = await s.readLibrary();
      expect(entries.map((e) => e.name), ['Mine']);
      expect(File(p.join(data.path, 'library.json')).existsSync(), isTrue, reason: 'import is persisted');

      final config = await s.readConfig();
      expect(config.language, 'German');
      expect(config.steamPath, '/s');

      // The legacy files are left untouched.
      expect(File(p.join(legacy.path, 'library.json')).existsSync(), isTrue);
    });

    test('writes are ignored before load and are atomic after', () async {
      final s = storage();
      final entry = TrainerEntry(id: 'x', name: 'A', trainerPath: '/t/a.exe');
      await s.writeLibrary([entry]);
      expect(File(p.join(data.path, 'library.json')).existsSync(), isFalse);

      await s.readLibrary();
      await s.writeLibrary([entry]);
      expect((await storage().readLibrary()).single.name, 'A');
      expect(data.listSync().map((e) => p.basename(e.path)), isNot(contains('library.json.tmp')));
    });

    test('a corrupt library is moved aside instead of being overwritten', () async {
      data.createSync(recursive: true);
      File(p.join(data.path, 'library.json')).writeAsStringSync('{broken');
      final s = storage();
      expect(await s.readLibrary(), isEmpty);
      await s.writeLibrary([TrainerEntry(id: 'n', name: 'New', trainerPath: '/t/n.exe')]);
      final names = data.listSync().map((e) => p.basename(e.path)).toList();
      expect(names.where((n) => n.startsWith('library.json.corrupt-')), hasLength(1));
      expect(names, contains('library.json'));
    });

    test('config round trip', () async {
      final s = storage();
      await s.readConfig();
      await s.writeConfig(language: '中文', steamPath: '/steam');
      final config = await storage().readConfig();
      expect(config.language, '中文');
      expect(config.steamPath, '/steam');
    });
  });
}
