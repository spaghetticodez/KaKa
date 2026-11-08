import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/util/game_info.dart';
import 'package:kaka/util/proton.dart';
import 'package:path/path.dart' as p;

const String manifest = '''
"AppState"
{
	"appid"		"1245620"
	"universe"		"1"
	"LauncherPath"		"/home/u/.local/share/Steam/ubuntu12_32/steam"
	"name"		"ELDEN RING \\"Deluxe\\" ®"
	"StateFlags"		"4"
	"UserConfig"
	{
		"name"		"not this one"
	}
}
''';

void main() {
  late Directory tmp;
  File touch(String path, {DateTime? modified}) {
    final f = File(path)
      ..createSync(recursive: true)
      ..writeAsStringSync('x');
    if (modified != null) f.setLastModifiedSync(modified);
    return f;
  }

  setUp(() => tmp = Directory.systemTemp.createTempSync('kaka_gameinfo_'));
  tearDown(() => tmp.deleteSync(recursive: true));

  test('appNameFromManifest reads the top-level name with escapes and non-ASCII', () {
    expect(appNameFromManifest(manifest), 'ELDEN RING "Deluxe" ®');
    expect(appNameFromManifest('"AppState" { "appid" "1" }'), isNull);
    expect(appNameFromManifest('"name" ""'), isNull);
  });

  group('findCover', () {
    late String root;
    late String cache;
    setUp(() {
      root = p.join(tmp.path, 'Steam');
      cache = p.join(root, 'appcache', 'librarycache', '1245620');
    });

    test('flat layout', () {
      touch(p.join(cache, 'header.jpg'));
      touch(p.join(cache, 'library_600x900.jpg'));
      expect(findCover(root, 1245620), p.join(cache, 'library_600x900.jpg'));
    });

    test('hashed sub-folders: the newest portrait capsule wins over headers', () {
      touch(p.join(cache, 'aaa', 'library_600x900.jpg'), modified: DateTime(2024));
      touch(p.join(cache, 'bbb', 'library_600x900.jpg'), modified: DateTime(2026));
      touch(p.join(cache, 'ccc', 'library_header.jpg'), modified: DateTime(2027));
      expect(findCover(root, 1245620), p.join(cache, 'bbb', 'library_600x900.jpg'));
    });

    test('falls back to the header; null without any artwork', () {
      touch(p.join(cache, 'ddd', 'library_header.jpg'));
      expect(findCover(root, 1245620), p.join(cache, 'ddd', 'library_header.jpg'));
      expect(findCover(root, 1), isNull);
    });
  });

  group('suggestionFor', () {
    test('Steam game on a second drive: name from that library, cover from the Steam root', () {
      final library = p.join(tmp.path, 'mnt', 'games', 'SteamLibrary', 'steamapps');
      File(p.join(library, 'appmanifest_1245620.acf'))
        ..createSync(recursive: true)
        ..writeAsStringSync(manifest);
      final root = p.join(tmp.path, 'Steam');
      final cover = touch(p.join(root, 'appcache', 'librarycache', '1245620', 'library_600x900.jpg')).path;
      final game =
          RunningGame(compatDataPath: p.join(library, 'compatdata', '1245620'), label: 'eldenring.exe', appId: 1245620, clientInstallPath: root);

      final s = suggestionFor(game, steamRoot: '/elsewhere');
      expect(s.name, 'ELDEN RING "Deluxe" ®');
      expect(s.appId, 1245620);
      expect(s.coverPath, cover, reason: 'the game\'s own Steam root wins over the configured one');
    });

    test('no manifest (non-Steam shortcut, Heroic): exe name without extension, no cover without AppID', () {
      final heroic = RunningGame(compatDataPath: p.join(tmp.path, 'Heroic', 'Prefixes', 'Cyberpunk'), label: 'Cyberpunk2077.exe');
      final s = suggestionFor(heroic, steamRoot: p.join(tmp.path, 'Steam'));
      expect(s.name, 'Cyberpunk2077');
      expect(s.appId, isNull);
      expect(s.coverPath, isNull);
    });
  });
}
