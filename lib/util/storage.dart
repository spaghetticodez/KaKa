import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';
import 'package:synchronized/synchronized.dart';

import 'dto.dart';

const String libraryFileName = 'library.json';
const String configFileName = 'config.json';

class AppConfig {
  final String? language;
  final String steamPath;

  const AppConfig({this.language, this.steamPath = ''});
}

/// JSON persistence in `$XDG_DATA_HOME/io.github.spaghetticodez.kaka/`.
///
/// Data lives in the data dir (not the cache dir, which cleaners wipe), writes
/// are atomic, and a LaLa library/config is imported once on first start.
class LocalStorage {
  LocalStorage({Future<Directory> Function()? dataDir, String? legacyDir})
      : _dataDirProvider = dataDir ?? getApplicationSupportDirectory,
        _legacyDirOverride = legacyDir;

  final Future<Directory> Function() _dataDirProvider;
  final String? _legacyDirOverride;
  final Lock _lock = Lock();

  /// Writes are ignored until the corresponding file was read, so a failed or
  /// pending load can never overwrite saved data with an empty state.
  bool libraryLoaded = false;
  bool configLoaded = false;

  Future<Directory> _dir() async {
    final dir = await _dataDirProvider();
    await dir.create(recursive: true);
    return dir;
  }

  String get _legacyDir {
    final override = _legacyDirOverride;
    if (override != null) return override;
    final cache = Platform.environment['XDG_CACHE_HOME'];
    final base = cache != null && cache.isNotEmpty ? cache : p.join(Platform.environment['HOME'] ?? '', '.cache');
    return p.join(base, 'com.aironheart.lala', 'config');
  }

  Future<List<TrainerEntry>> readLibrary() async {
    final file = File(p.join((await _dir()).path, libraryFileName));
    if (await file.exists()) {
      final entries = await _readJson(file, _parseLibrary) ?? <TrainerEntry>[];
      libraryLoaded = true;
      return entries;
    }
    final legacy = File(p.join(_legacyDir, libraryFileName));
    final imported = await legacy.exists() ? await _readJson(legacy, _parseLibrary, quarantine: false) ?? <TrainerEntry>[] : <TrainerEntry>[];
    libraryLoaded = true;
    if (imported.isNotEmpty) await writeLibrary(imported);
    return imported;
  }

  Future<void> writeLibrary(List<TrainerEntry> entries) async {
    if (!libraryLoaded) return;
    final file = File(p.join((await _dir()).path, libraryFileName));
    await _atomicWrite(file, const JsonEncoder.withIndent('  ').convert(entries.map((e) => e.toJson()).toList()));
  }

  Future<AppConfig> readConfig() async {
    final file = File(p.join((await _dir()).path, configFileName));
    final ownConfig = await file.exists();
    final source = ownConfig ? file : File(p.join(_legacyDir, configFileName));
    final config = await source.exists() ? await _readJson(source, _parseConfig, quarantine: ownConfig) : null;
    configLoaded = true;
    return config ?? const AppConfig();
  }

  Future<void> writeConfig({required String language, required String steamPath}) async {
    if (!configLoaded) return;
    final file = File(p.join((await _dir()).path, configFileName));
    await _atomicWrite(file, const JsonEncoder.withIndent('  ').convert({'language': language, 'custom_steam_path': steamPath}));
  }

  static List<TrainerEntry> _parseLibrary(Object? json) =>
      (json as List<dynamic>).whereType<Map<String, dynamic>>().map(TrainerEntry.fromJson).whereType<TrainerEntry>().toList();

  static AppConfig _parseConfig(Object? json) {
    final map = json as Map<String, dynamic>;
    final language = map['language'];
    final steamPath = map['custom_steam_path'];
    return AppConfig(language: language is String ? language : null, steamPath: steamPath is String ? steamPath : '');
  }

  /// Parses [file]; on corrupt content the file is renamed aside (when
  /// [quarantine]) so the next save does not destroy it.
  Future<T?> _readJson<T>(File file, T Function(Object? json) parse, {bool quarantine = true}) async {
    try {
      return parse(jsonDecode(await file.readAsString()));
    } catch (e) {
      debugPrint('Could not read ${file.path}: $e');
      if (quarantine) {
        try {
          await file.rename('${file.path}.corrupt-${DateTime.now().millisecondsSinceEpoch}');
        } on FileSystemException {
          // Leave it in place; the next save replaces it.
        }
      }
      return null;
    }
  }

  Future<void> _atomicWrite(File file, String content) => _lock.synchronized(() async {
        final tmp = File('${file.path}.tmp');
        await tmp.writeAsString(content, flush: true);
        await tmp.rename(file.path);
      });
}
