// Test doubles for the UI-facing state: storage kept in memory and a launcher
// that records launches instead of starting Proton.
import 'dart:io';

import 'package:kaka/l10n/app_localizations.dart';
import 'package:kaka/util/dto.dart';
import 'package:kaka/util/game_launcher.dart';
import 'package:kaka/util/storage.dart';

class MemoryStorage extends LocalStorage {
  MemoryStorage({List<TrainerEntry>? library, this.config = const AppConfig()}) : library = [...?library];

  List<TrainerEntry> library;
  AppConfig config;
  int libraryWrites = 0;
  int configWrites = 0;

  @override
  Future<List<TrainerEntry>> readLibrary() async => [...library];

  @override
  Future<void> writeLibrary(List<TrainerEntry> entries) async {
    library = [...entries];
    libraryWrites++;
  }

  @override
  Future<AppConfig> readConfig() async => config;

  @override
  Future<void> writeConfig({required String language, required String steamPath}) async {
    config = AppConfig(language: language, steamPath: steamPath);
    configWrites++;
  }
}

class FakeLauncher extends TrainerLauncher {
  FakeLauncher({Directory? logDir}) : super(logDir: logDir ?? Directory.systemTemp);

  /// Thrown by the next [launch] calls when set.
  Object? failure;
  final List<String> launched = [];
  int stopAllCalls = 0;

  @override
  Future<void> launch(TrainerEntry entry, AppLocalizations l10n) async {
    launched.add(entry.id);
    final error = failure;
    if (error != null) throw error;
  }

  @override
  Future<void> stopAll() async => stopAllCalls++;
}

TrainerEntry trainer(String id, {String? path, int? appId}) =>
    TrainerEntry(id: id, name: 'Game $id', trainerPath: path ?? '/t/$id.exe', steamAppId: appId);
