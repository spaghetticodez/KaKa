import 'dart:io';

import 'package:path/path.dart' as p;

/// Reset of the state FLiNG trainers keep inside a game's Proton prefix.
///
/// FLiNG trainers refuse to work ("previous trainer instance did not exit
/// properly") when stale state is left in `%LocalAppData%\FLiNGTrainer` or
/// `%LocalAppData%\Temp` (upstream LaLa #78; clearing both fixed it). Nothing
/// else is touched; in particular not Proton's `pfx.lock`, which is a flock
/// lock whose file may exist at any time.
class TrainerDataCleanup {
  TrainerDataCleanup(this.compatDataPath);

  /// The game's prefix (`.../compatdata/<id>`), as recorded at launch.
  final String compatDataPath;

  /// `%LocalAppData%` of the Proton user.
  String get localAppData => p.join(compatDataPath, 'pfx', 'drive_c', 'users', 'steamuser', 'AppData', 'Local');

  /// FLiNG's own folder: removed completely.
  String get flingDir => p.join(localAppData, 'FLiNGTrainer');

  /// Shared temp folder: emptied, the folder itself stays.
  String get tempDir => p.join(localAppData, 'Temp');

  List<String> get targets => [flingDir, tempDir];

  /// Deletes the targets and returns how many top-level items were removed.
  ///
  /// Refuses with [CleanupRefused] when [inUse] (game or trainer still running
  /// in the prefix) or when `%LocalAppData%` resolves outside the prefix.
  /// Symlinks are removed as links; their targets are never followed.
  int run({required bool inUse}) {
    if (inUse) throw const CleanupRefused(CleanupRefusal.prefixInUse);
    if (!Directory(localAppData).existsSync()) return 0;

    final prefixReal = Directory(compatDataPath).resolveSymbolicLinksSync();
    final localReal = Directory(localAppData).resolveSymbolicLinksSync();
    if (!p.isWithin(prefixReal, localReal)) throw const CleanupRefused(CleanupRefusal.outsidePrefix);

    var removed = 0;
    switch (FileSystemEntity.typeSync(flingDir, followLinks: false)) {
      case FileSystemEntityType.directory:
        Directory(flingDir).deleteSync(recursive: true);
        removed++;
      case FileSystemEntityType.notFound:
        break;
      default:
        _deleteEntry(flingDir);
        removed++;
    }

    if (FileSystemEntity.typeSync(tempDir, followLinks: false) == FileSystemEntityType.directory) {
      for (final entry in Directory(tempDir).listSync(followLinks: false)) {
        try {
          _deleteEntry(entry.path);
          removed++;
        } on FileSystemException {
          // Best effort: a file that cannot be removed does not block the rest.
        }
      }
    }
    return removed;
  }

  static void _deleteEntry(String path) {
    switch (FileSystemEntity.typeSync(path, followLinks: false)) {
      case FileSystemEntityType.link:
        Link(path).deleteSync();
      case FileSystemEntityType.directory:
        // Recursive delete removes links inside the tree without following them.
        Directory(path).deleteSync(recursive: true);
      case FileSystemEntityType.notFound:
        break;
      default:
        File(path).deleteSync();
    }
  }
}

enum CleanupRefusal { prefixInUse, outsidePrefix }

class CleanupRefused implements Exception {
  const CleanupRefused(this.reason);

  final CleanupRefusal reason;

  @override
  String toString() => 'CleanupRefused($reason)';
}
