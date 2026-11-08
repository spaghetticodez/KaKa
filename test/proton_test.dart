import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/util/proton.dart';
import 'package:path/path.dart' as p;

List<int> nul(List<String> parts) => utf8.encode('${parts.join('\u0000')}\u0000');

ProcessInfo proc(int pid, Map<String, String> env, List<String> cmdline) => ProcessInfo(pid, env, cmdline);

void main() {
  group('parseEnviron / parseCmdline', () {
    test('splits NUL separated blobs, keeps "=" inside values', () {
      final env = parseEnviron(nul(['A=1', 'STEAM_COMPAT_DATA_PATH=/x/compatdata/42', 'WEIRD=a=b', 'NOEQUALS']));
      expect(env, {'A': '1', 'STEAM_COMPAT_DATA_PATH': '/x/compatdata/42', 'WEIRD': 'a=b'});
      expect(parseCmdline(nul(['python3', '/p/proton', 'waitforexitandrun', 'game.exe'])), ['python3', '/p/proton', 'waitforexitandrun', 'game.exe']);
    });

    test('handles non-UTF-8 bytes without throwing', () {
      expect(parseEnviron([0x41, 0x3d, 0xff, 0x00]).containsKey('A'), isTrue);
    });
  });

  group('detectRunningGames', () {
    const steamGameEnv = {
      'STEAM_COMPAT_DATA_PATH': '/mnt/games/SteamLibrary/steamapps/compatdata/1245620/',
      'STEAM_COMPAT_CLIENT_INSTALL_PATH': '/home/u/.local/share/Steam',
      'SteamAppId': '1245620',
    };

    test('Steam game on a second drive: prefix, Steam root, AppID and proton script come from the environment', () {
      final games = detectRunningGames([
        proc(10, steamGameEnv, ['/home/u/.local/share/Steam/ubuntu12_32/reaper', 'SteamLaunch', 'AppId=1245620']),
        proc(11, steamGameEnv, ['python3', '/home/u/.local/share/Steam/compatibilitytools.d/GE-Proton9-20/proton', 'waitforexitandrun', '/mnt/games/x/eldenring.exe']),
        proc(12, steamGameEnv, ['C:\\windows\\system32\\steam.exe', 'Z:\\mnt\\games\\x\\eldenring.exe']),
        proc(13, steamGameEnv, ['Z:\\mnt\\games\\x\\eldenring.exe']),
      ]);
      expect(games, hasLength(1));
      final g = games.single;
      expect(g.compatDataPath, '/mnt/games/SteamLibrary/steamapps/compatdata/1245620');
      expect(g.clientInstallPath, '/home/u/.local/share/Steam');
      expect(g.appId, 1245620);
      expect(g.protonScript, '/home/u/.local/share/Steam/compatibilitytools.d/GE-Proton9-20/proton');
      expect(g.label, 'eldenring.exe');
    });

    test('Heroic/umu game without a Steam AppID is still detected', () {
      final games = detectRunningGames([
        proc(20, {'STEAM_COMPAT_DATA_PATH': '/home/u/Games/Heroic/Prefixes/default/Cyberpunk 2077', 'STEAM_COMPAT_CLIENT_INSTALL_PATH': ''}, [
          'python3',
          '/home/u/.var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton/GE-Proton-latest/proton',
          'run',
          '/home/u/Games/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe',
        ]),
      ]);
      expect(games.single.appId, isNull);
      expect(games.single.clientInstallPath, isNull, reason: 'empty value is treated as unset');
      expect(games.single.label, 'Cyberpunk2077.exe');
    });

    test('ignores processes started by KaKa and processes without a prefix', () {
      final games = detectRunningGames([
        proc(30, {...steamGameEnv, trainerMarkerEnv: '1'}, ['Z:\\trainers\\EldenRingTrainer.exe']),
        proc(31, {'HOME': '/home/u'}, ['/usr/bin/bash']),
        proc(32, {'STEAM_COMPAT_DATA_PATH': '/x/compatdata/7'}, ['/usr/bin/sleep', '100']),
      ]);
      expect(games, isEmpty);
    });

    test('several games are reported separately; matchGame picks by AppID or prefix folder', () {
      final games = detectRunningGames([
        proc(40, steamGameEnv, ['Z:\\a\\eldenring.exe']),
        proc(41, {'STEAM_COMPAT_DATA_PATH': '/home/u/.local/share/Steam/steamapps/compatdata/3123456789'}, ['C:\\x\\shortcut.exe']),
      ]);
      expect(games, hasLength(2));
      expect(matchGame(games, 1245620)?.label, 'eldenring.exe');
      expect(matchGame(games, 3123456789)?.label, 'shortcut.exe');
      expect(matchGame(games, 999), isNull);
    });

    test('Wine helper executables are not used as the game label', () {
      final games = detectRunningGames([
        proc(50, {'STEAM_COMPAT_DATA_PATH': '/c/compatdata/5'}, ['C:\\windows\\system32\\services.exe']),
        proc(51, {'STEAM_COMPAT_DATA_PATH': '/c/compatdata/5'}, ['C:\\Games\\Real.exe']),
      ]);
      expect(games.single.label, 'Real.exe');
    });
  });

  group('protonDirFromConfigInfo', () {
    bool Function(String) filesAt(Set<String> files) => (path) => files.contains(path);

    test('Steam compatibilitytools.d build with a space in its name', () {
      const info = '11.0-100\n/home/u/.local/share/Steam/compatibilitytools.d/Proton-GE Latest/files/share/fonts/\n'
          '/home/u/.local/share/Steam/compatibilitytools.d/Proton-GE Latest/files/lib/\n';
      expect(protonDirFromConfigInfo(info, filesAt({'/home/u/.local/share/Steam/compatibilitytools.d/Proton-GE Latest/proton'})),
          '/home/u/.local/share/Steam/compatibilitytools.d/Proton-GE Latest');
    });

    test('Valve Proton in steamapps/common', () {
      const info = '11.0-100\n/s/steamapps/common/Proton - Experimental/files/share/fonts/\n';
      expect(protonDirFromConfigInfo(info, filesAt({'/s/steamapps/common/Proton - Experimental/proton'})), '/s/steamapps/common/Proton - Experimental');
    });

    test('Heroic: parent folder named "proton" is not mistaken for the install (upstream #88)', () {
      const base = '/var/home/u/.var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton/GE-Proton-latest';
      const info = 'GE-Proton10-4\n$base/files/share/fonts/\n';
      expect(protonDirFromConfigInfo(info, filesAt({'$base/proton'})), base);
    });

    test('home directory containing "proton" does not confuse the lookup', () {
      const info = 'x\n/home/protonfan/.steam/steam/compatibilitytools.d/GE-Proton9-1/files/share/fonts/\n';
      expect(protonDirFromConfigInfo(info, filesAt({'/home/protonfan/.steam/steam/compatibilitytools.d/GE-Proton9-1/proton'})),
          '/home/protonfan/.steam/steam/compatibilitytools.d/GE-Proton9-1');
    });

    test('returns null when the Proton build was removed, or for empty input', () {
      expect(protonDirFromConfigInfo('x\n/gone/GE-Proton8-1/files/share/fonts/\n', filesAt({})), isNull);
      expect(protonDirFromConfigInfo('', filesAt({})), isNull);
    });
  });

  group('Steam root', () {
    test('findSteamRoot: user override wins and is not silently replaced', () {
      final dirs = {'/home/u/.local/share/Steam/steamapps'};
      expect(findSteamRoot('/home/u', dirExists: dirs.contains), '/home/u/.local/share/Steam');
      expect(findSteamRoot('/home/u', override: '/custom', dirExists: dirs.contains), isNull);
      expect(findSteamRoot('/home/u', override: '/custom', dirExists: {'/custom/steamapps'}.contains), '/custom');
    });

    test('findSteamRoot finds Flatpak Steam', () {
      final dirs = {'/home/u/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps'};
      expect(findSteamRoot('/home/u', dirExists: dirs.contains), '/home/u/.var/app/com.valvesoftware.Steam/.local/share/Steam');
    });
  });

  group('trainerVerb', () {
    test('runinprefix when the Proton build supports it, run for older builds', () {
      expect(trainerVerb(protonScript: 'elif sys.argv[1] == "runinprefix":'), 'runinprefix');
      expect(trainerVerb(protonScript: 'if sys.argv[1] == "run":'), 'run');
      expect(trainerVerb(protonScript: ''), 'run');
    });
  });

  group('prefixInUse', () {
    const compat = '/games/steamapps/compatdata/1245620';
    test('matches STEAM_COMPAT_DATA_PATH (any spelling) and WINEPREFIX, including KaKa trainers', () {
      expect(prefixInUse([proc(1, {'STEAM_COMPAT_DATA_PATH': '$compat/'}, ['game.exe'])], compat), isTrue);
      expect(prefixInUse([proc(2, {'WINEPREFIX': '$compat/pfx'}, ['wineserver'])], compat), isTrue);
      expect(prefixInUse([proc(3, {'STEAM_COMPAT_DATA_PATH': compat, trainerMarkerEnv: '1'}, ['Trainer.exe'])], compat), isTrue);
    });
    test('other prefixes and unrelated processes do not count', () {
      expect(prefixInUse([proc(4, {'STEAM_COMPAT_DATA_PATH': '${compat}0'}, ['x']), proc(5, {'WINEPREFIX': compat}, ['y']), proc(6, {}, ['z'])], compat),
          isFalse);
    });
  });

  group('readProcesses', () {
    late Directory root;

    setUp(() => root = Directory.systemTemp.createTempSync('kaka_proc_'));
    tearDown(() => root.deleteSync(recursive: true));

    test('reads numeric entries, skips unreadable ones and non-pid folders', () {
      final good = Directory(p.join(root.path, '123'))..createSync();
      File(p.join(good.path, 'environ')).writeAsBytesSync(nul(['STEAM_COMPAT_DATA_PATH=/c/compatdata/9']));
      File(p.join(good.path, 'cmdline')).writeAsBytesSync(nul(['C:\\g\\game.exe']));
      Directory(p.join(root.path, '456')).createSync(); // vanished process: no files
      Directory(p.join(root.path, 'self')).createSync();

      final procs = readProcesses(procRoot: root.path);
      expect(procs.map((e) => e.pid), [123]);
      expect(detectRunningGames(procs).single.label, 'game.exe');
    });

    test('works against the real /proc of this machine', () {
      expect(readProcesses().any((e) => e.pid == pid), isTrue);
    });
  });
}
