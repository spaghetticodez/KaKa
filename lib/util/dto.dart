import 'dart:math';

/// A user-added trainer. KaKa never downloads trainers: every entry points at a local `.exe`.
class TrainerEntry {
  final String id;
  String name;
  String trainerPath;
  String coverImagePath;

  /// Optional Steam AppID of the target game. Picks the right game when several
  /// Proton games are running; the game itself must already run.
  int? steamAppId;

  /// Proton prefix (`.../compatdata/<id>`) of the last successful launch, so
  /// trainer data can be reset there while the game is not running. Empty if
  /// the trainer was never launched.
  String lastPrefix;

  TrainerEntry({required this.id, required this.name, required this.trainerPath, this.coverImagePath = '', this.steamAppId, this.lastPrefix = ''});

  static String newId() {
    final random = Random.secure();
    return List.generate(16, (_) => random.nextInt(16).toRadixString(16)).join();
  }

  /// Returns null for entries that are unusable offline, such as LaLa
  /// online-catalogue games (they have no local `trainer_path`).
  static TrainerEntry? fromJson(Map<String, dynamic> json) {
    final name = json['name'];
    final trainerPath = json['trainer_path'];
    if (name is! String || trainerPath is! String || trainerPath.isEmpty) return null;
    final id = json['id'];
    final cover = json['cover_image_path'];
    final appId = json['steam_app_id'];
    final lastPrefix = json['last_prefix'];
    return TrainerEntry(
      // Legacy LaLa custom games have a random numeric app_id instead of an id.
      id: id is String && id.isNotEmpty ? id : 'lala-${json['app_id'] ?? newId()}',
      name: name,
      trainerPath: trainerPath,
      coverImagePath: cover is String ? cover : '',
      steamAppId: appId is int && appId > 0 ? appId : null,
      lastPrefix: lastPrefix is String ? lastPrefix : '',
    );
  }

  Map<String, dynamic> toJson() => {
        'id': id,
        'name': name,
        'trainer_path': trainerPath,
        'cover_image_path': coverImagePath,
        'steam_app_id': steamAppId,
        'last_prefix': lastPrefix,
      };
}
