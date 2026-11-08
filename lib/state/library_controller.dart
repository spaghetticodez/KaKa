import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:url_launcher/url_launcher.dart';

import '../l10n/app_localizations.dart';
import '../util/dto.dart';
import '../util/game_launcher.dart';
import '../util/language.dart';
import '../util/storage.dart';

/// Opens a local file or folder in the desktop's default application.
typedef UriOpener = Future<bool> Function(Uri uri);

/// App state: the trainer library and settings. UI-free, so it is unit-testable;
/// widgets listen to it and call its methods.
class LibraryController extends ChangeNotifier {
  LibraryController({required this.storage, required this.launcher, UriOpener? openUri}) : _openUri = openUri ?? launchUrl;

  final LocalStorage storage;
  final TrainerLauncher launcher;
  final UriOpener _openUri;

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
  /// the failure with a user-facing message.
  Future<LaunchException?> launch(TrainerEntry entry, AppLocalizations l10n) async {
    _trainers.removeWhere((e) => e.id == entry.id);
    _trainers.insert(0, entry);
    notifyListeners();
    await _saveLibrary();
    try {
      await launcher.launch(entry, l10n);
      return null;
    } on LaunchException catch (e) {
      return e;
    } catch (e) {
      debugPrint('Launch failed: $e');
      return LaunchException(e.toString());
    }
  }

  /// Folder with the per-launch trainer logs.
  String get logDir => launcher.logDir.path;

  /// Opens one launch log in the default text viewer.
  Future<bool> openLog(String path) => _open(Uri.file(path));

  /// Opens the log folder in the file manager (created if no trainer ran yet).
  Future<bool> openLogFolder() {
    try {
      launcher.logDir.createSync(recursive: true);
    } on FileSystemException catch (e) {
      debugPrint('Could not create ${launcher.logDir.path}: $e');
    }
    return _open(Uri.directory(launcher.logDir.path));
  }

  Future<bool> _open(Uri uri) async {
    try {
      return await _openUri(uri);
    } catch (e) {
      debugPrint('Could not open $uri: $e');
      return false;
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
