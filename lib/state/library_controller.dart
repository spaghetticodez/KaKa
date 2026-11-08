import 'package:flutter/foundation.dart';

import '../l10n/app_localizations.dart';
import '../util/dto.dart';
import '../util/game_launcher.dart';
import '../util/language.dart';
import '../util/storage.dart';

/// App state: the trainer library and settings. UI-free, so it is unit-testable;
/// widgets listen to it and call its methods.
class LibraryController extends ChangeNotifier {
  LibraryController({required this.storage, required this.launcher});

  final LocalStorage storage;
  final TrainerLauncher launcher;

  List<TrainerEntry> _trainers = [];
  bool _loading = true;
  String _language = languageOptions.first;

  List<TrainerEntry> get trainers => List.unmodifiable(_trainers);

  bool get loading => _loading;

  /// Display name from [languageOptions].
  String get language => _language;

  /// Steam install root chosen in Settings; empty means auto-detect.
  String get steamPath => launcher.steamPath;

  Future<void> load() async {
    final config = await storage.readConfig();
    final language = config.language;
    if (language != null && languageOptions.contains(language)) _language = language;
    launcher.steamPath = config.steamPath;
    _trainers = await storage.readLibrary();
    _loading = false;
    notifyListeners();
  }

  Future<void> add(TrainerEntry entry) {
    _trainers.insert(0, entry);
    notifyListeners();
    return _saveLibrary();
  }

  Future<void> update(TrainerEntry entry) {
    final index = _trainers.indexWhere((e) => e.id == entry.id);
    if (index == -1) return Future.value();
    _trainers[index] = entry;
    notifyListeners();
    return _saveLibrary();
  }

  Future<void> remove(String id) {
    _trainers.removeWhere((e) => e.id == id);
    notifyListeners();
    return _saveLibrary();
  }

  Future<void> setLanguage(String language) {
    if (!languageOptions.contains(language) || language == _language) return Future.value();
    _language = language;
    notifyListeners();
    return _saveConfig();
  }

  Future<void> setSteamPath(String path) {
    launcher.steamPath = path;
    notifyListeners();
    return _saveConfig();
  }

  /// Moves [entry] to the front and launches it. Returns null on success, or
  /// the user-facing error message.
  Future<String?> launch(TrainerEntry entry, AppLocalizations l10n) async {
    _trainers.removeWhere((e) => e.id == entry.id);
    _trainers.insert(0, entry);
    notifyListeners();
    await _saveLibrary();
    try {
      await launcher.launch(entry, l10n);
      return null;
    } on LaunchException catch (e) {
      return e.message;
    } catch (e) {
      debugPrint('Launch failed: $e');
      return e.toString();
    }
  }

  /// Saves everything and stops the trainers this session started.
  Future<void> shutdown() async {
    await _saveConfig();
    await _saveLibrary();
    await launcher.stopAll();
  }

  Future<void> _saveLibrary() => storage.writeLibrary(_trainers);

  Future<void> _saveConfig() => storage.writeConfig(language: _language, steamPath: launcher.steamPath);
}
