import 'dart:io';

import 'package:path/path.dart' as p;

import 'proton.dart';

/// What KaKa can fill in for a running game from local data only: the game's
/// Steam manifest and Steam's own artwork cache. Nothing is downloaded.
class GameSuggestion {
  const GameSuggestion({required this.name, this.appId, this.coverPath});

  final String name;
  final int? appId;

  /// Local cover image from Steam's library cache, if one exists.
  final String? coverPath;
}

/// The app name from an `appmanifest_<appid>.acf` (Valve KeyValues text).
String? appNameFromManifest(String acf) {
  final match = RegExp(r'"name"\s+"((?:[^"\\]|\\.)*)"').firstMatch(acf);
  final name = match?.group(1)?.replaceAllMapped(RegExp(r'\\(.)'), (m) => m.group(1)!).trim();
  return name == null || name.isEmpty ? null : name;
}

/// Portrait cover for [appId] in `<steamRoot>/appcache/librarycache`.
///
/// Steam keeps artwork either directly in `librarycache/<appid>/` or in hashed
/// sub-folders (`librarycache/<appid>/<hash>/`); the newest copy wins. The
/// 600x900 capsule matches KaKa's 2:3 cards; the landscape header is a fallback.
String? findCover(String steamRoot, int appId) {
  final dir = Directory(p.join(steamRoot, 'appcache', 'librarycache', '$appId'));
  if (!dir.existsSync()) return null;
  final files = <File>[];
  try {
    for (final entry in dir.listSync(recursive: true, followLinks: false)) {
      if (entry is File && p.split(p.relative(entry.path, from: dir.path)).length <= 2) files.add(entry);
    }
  } on FileSystemException {
    return null;
  }
  for (final name in const ['library_600x900_2x.jpg', 'library_600x900.jpg', 'library_header.jpg', 'header.jpg']) {
    final matches = files.where((f) => p.basename(f.path) == name).toList();
    if (matches.isEmpty) continue;
    matches.sort((a, b) => b.statSync().modified.compareTo(a.statSync().modified));
    return matches.first.path;
  }
  return null;
}

/// Name, AppID and cover for [game]. The manifest is looked up next to the
/// game's own prefix (`<library>/steamapps/compatdata/<id>` →
/// `<library>/steamapps/appmanifest_<id>.acf`), so games on any drive work;
/// games without one (non-Steam shortcuts, Heroic, ...) use the exe name.
GameSuggestion suggestionFor(RunningGame game, {String? steamRoot}) {
  final appId = game.appId;
  String? name;
  if (appId != null) {
    final manifest = File(p.join(p.dirname(p.dirname(game.compatDataPath)), 'appmanifest_$appId.acf'));
    try {
      if (manifest.existsSync()) name = appNameFromManifest(manifest.readAsStringSync());
    } on FileSystemException {
      // Fall back to the executable name.
    }
  }
  final root = game.clientInstallPath ?? steamRoot;
  return GameSuggestion(
    name: name ?? p.basenameWithoutExtension(game.label),
    appId: appId,
    coverPath: appId != null && root != null && root.isNotEmpty ? findCover(root, appId) : null,
  );
}
