import 'dart:async';
import 'dart:io';
import 'dart:isolate';

import 'package:flutter/foundation.dart';
import 'package:url_launcher/url_launcher.dart';

import '../l10n/app_localizations.dart';
import '../util/dto.dart';
import '../util/game_info.dart';
import '../util/game_launcher.dart';
import '../util/language.dart';
import '../util/proton.dart';
import '../util/storage.dart';
import '../util/trainer_data.dart';

/// Opens a local file or folder in the desktop's default application.
typedef UriOpener = Future<bool> Function(Uri uri);

/// Reads the process list in a background isolate, so the ~10-20 ms scan of
/// /proc never blocks a frame.
Future<List<ProcessInfo>> scanProcessesInBackground() => Isolate.run(readProcesses);

/// App state: the trainer library and settings. UI-free, so it is unit-testable;
/// widgets listen to it and call its methods.
class LibraryController extends ChangeNotifier {
  LibraryController({
    required this.storage,
    required this.launcher,
    UriOpener? openUri,
    Future<List<ProcessInfo>> Function()? processes,
    this.orphanPollInterval = const Duration(seconds: 5),
  })  : _openUri = openUri ?? launchUrl,
        _scan = processes ?? scanProcessesInBackground {
    _launcherChanges = launcher.changes.listen((_) => notifyListeners());
  }

  final LocalStorage storage;
  final TrainerLauncher launcher;
  final UriOpener _openUri;
  final Future<List<ProcessInfo>> Function() _scan;

  /// How often trainers left running by an earlier session are re-checked
  /// (only while there are any).
  final Duration orphanPollInterval;

  List<TrainerEntry> _trainers = [];
  bool _loading = true;
  String _language = languageOptions.first;

  /// Running trainers this session did not start (found by a scan).
  Set<String> _orphans = {};
  Timer? _orphanPoll;
  late final StreamSubscription<void> _launcherChanges;
  Future<List<ProcessInfo>>? _pendingScan;
  bool _disposed = false;

  List<TrainerEntry> get trainers => List.unmodifiable(_trainers);

  bool get loading => _loading;

  /// Whether the trainer of entry [id] runs: started by this session and its
  /// Proton wrapper still alive (event-driven), or found by the last scan.
  bool isRunning(String id) => launcher.liveIds.contains(id) || _orphans.contains(id);

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
  /// the failure with a user-facing message. A trainer that is already running
  /// is not started a second time (FLiNG trainers fail on a second instance).
  Future<LaunchException?> launch(TrainerEntry entry, AppLocalizations l10n) async {
    final processes = await rescan();
    if (isRunning(entry.id)) return LaunchException(l10n.trainerAlreadyRunning(entry.name));
    _trainers.removeWhere((e) => e.id == entry.id);
    _trainers.insert(0, entry);
    notifyListeners();
    await _saveLibrary();
    try {
      final plan = await launcher.launch(entry, l10n, processes: processes);
      entry.lastPrefix = plan.compatDataPath;
      notifyListeners();
      await _saveLibrary();
      return null;
    } on LaunchException catch (e) {
      return e;
    } catch (e) {
      debugPrint('Launch failed: $e');
      return LaunchException(e.toString());
    }
  }

  /// One scan at startup, to show trainers an earlier session left running.
  /// Trainers started from now on are tracked through their process exit.
  Future<void> startMonitoring() async {
    await rescan();
  }

  /// Scans the process list once (in the background) and updates the set of
  /// running trainers this session did not start. Concurrent calls share one
  /// scan. Returns the scanned processes.
  Future<List<ProcessInfo>> rescan() {
    return _pendingScan ??= () async {
      try {
        final processes = await _scan();
        if (_disposed) return processes;
        final orphans = launcher.runningIds(_trainers, processes).difference(launcher.liveIds);
        if (!setEquals(orphans, _orphans)) {
          _orphans = orphans;
          notifyListeners();
        }
        // Orphans have no exit event to wait for: re-check them, but only while any exist.
        if (_orphans.isEmpty) {
          _orphanPoll?.cancel();
          _orphanPoll = null;
        } else {
          _orphanPoll ??= Timer.periodic(orphanPollInterval, (_) => rescan());
        }
        return processes;
      } finally {
        _pendingScan = null;
      }
    }();
  }

  /// Stops [entry]'s trainer and waits up to [timeout] for it to exit.
  /// Returns whether it is gone.
  Future<bool> stop(TrainerEntry entry, {Duration timeout = const Duration(seconds: 5)}) async {
    launcher.stop(entry, await _scan());
    final deadline = DateTime.now().add(timeout);
    while (true) {
      await rescan();
      if (!isRunning(entry.id)) return true;
      if (DateTime.now().isAfter(deadline)) return false;
      await Future<void>.delayed(const Duration(milliseconds: 300));
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _orphanPoll?.cancel();
    _launcherChanges.cancel();
    super.dispose();
  }

  /// Proton games running right now, with name, AppID and cover filled in from
  /// local Steam data where available (for "use running game" when adding).
  Future<List<GameSuggestion>> runningGames() async {
    final games = detectRunningGames(await _scan());
    if (games.isEmpty) return const [];
    final root = launcher.steamRoot();
    return [for (final g in games) suggestionFor(g, steamRoot: root)];
  }

  /// Resets FLiNG trainer state in the prefix [entry] last ran in, see
  /// [TrainerDataCleanup]. Returns the number of removed items. Throws
  /// [CleanupRefused] while anything still runs in that prefix.
  Future<int> resetTrainerData(TrainerEntry entry) async {
    final prefix = entry.lastPrefix;
    if (prefix.isEmpty) return 0;
    final inUse = prefixInUse(await _scan(), prefix);
    return TrainerDataCleanup(prefix).run(inUse: inUse);
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
