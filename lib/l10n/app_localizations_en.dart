// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get title => 'KaKa Trainer Launcher';

  @override
  String get launch => 'Launch';

  @override
  String get libraryEmpty => 'No trainers yet.\nClick + to add a trainer (.exe) for one of your games.';

  @override
  String get addTrainer => 'Add Trainer';

  @override
  String get editTrainer => 'Edit Trainer';

  @override
  String get edit => 'Edit';

  @override
  String get gameName => 'Game name';

  @override
  String get trainerFile => 'Trainer (.exe)';

  @override
  String get gameCover => 'Cover image (optional)';

  @override
  String get steamAppId => 'Steam AppID (optional)';

  @override
  String get steamAppIdHelp => 'Only needed when several games run at once: picks the right one.';

  @override
  String get select => 'Select';

  @override
  String get change => 'Change';

  @override
  String get save => 'Save';

  @override
  String get cancel => 'Cancel';

  @override
  String get remove => 'Remove';

  @override
  String removeTrainerConfirm(Object name) {
    return 'Remove \"$name\" from the library?\n\nThe trainer file itself is not deleted.';
  }

  @override
  String get trainerAdded => 'Trainer added';

  @override
  String get trainerUpdated => 'Trainer updated';

  @override
  String get requiredFields => 'Game name and trainer file are required.';

  @override
  String get invalidAppId => 'Steam AppID must be a positive number.';

  @override
  String get setting => 'Settings';

  @override
  String get language => 'Language';

  @override
  String get steamPath => 'Steam path';

  @override
  String get steamPathAuto => 'Auto-detect';

  @override
  String get set => 'Set';

  @override
  String get reset => 'Reset';

  @override
  String get steamPathSet => 'Steam path set';

  @override
  String get ok => 'OK';

  @override
  String get launchFail => 'Launch failed';

  @override
  String launched(Object name) {
    return 'Trainer started: $name';
  }

  @override
  String trainerPathNotFound(Object trainerPath) {
    return 'Trainer file not found.\n\nPath: $trainerPath';
  }

  @override
  String get gameNotRunning =>
      'No running Proton game found.\n\nStart the game first (Steam, Heroic, Lutris or another Proton launcher), then launch the trainer.';

  @override
  String multipleGamesRunning(Object games) {
    return 'Several Proton games are running:\n\n$games\n\nSet the Steam AppID on this trainer to choose one.';
  }

  @override
  String get gameStarting => 'The game is still starting (its Proton prefix is not ready yet).\n\nTry again in a few seconds.';

  @override
  String protonPathNotFound(Object prefix) {
    return 'Proton installation for this game not found.\n\nPrefix: $prefix';
  }

  @override
  String trainerExited(Object code, Object logPath, Object output) {
    return 'The trainer exited right away (exit code $code).\n\n$output\n\nFull log: $logPath';
  }

  @override
  String get sourceCode => 'Source';

  @override
  String get basedOn => 'Based on LaLa';

  @override
  String get openLog => 'Open log';

  @override
  String get trainerLogs => 'Trainer logs';

  @override
  String get openFolder => 'Open folder';

  @override
  String get resetTrainerData => 'Reset trainer data';

  @override
  String resetTrainerDataConfirm(Object paths) {
    return 'For FLiNG trainers that report \"previous trainer instance did not exit properly\".\n\nClose the game and the trainer first. KaKa then deletes the FLiNGTrainer folder and empties the Temp folder of the game prefix:\n\n$paths\n\nNothing else is changed.';
  }

  @override
  String resetTrainerDataDone(Object count) {
    return 'Trainer data reset ($count items removed)';
  }

  @override
  String get prefixInUse => 'The game or a trainer is still running in this prefix. Close them first, then try again.';

  @override
  String get cleanupOutsidePrefix => 'Not reset: the AppData folder of this prefix points outside the prefix.';

  @override
  String get running => 'Running';

  @override
  String get stop => 'Stop';

  @override
  String trainerAlreadyRunning(Object name) {
    return '$name is already running.';
  }

  @override
  String trainerStopped(Object name) {
    return 'Trainer stopped: $name';
  }

  @override
  String trainerStopFailed(Object name) {
    return '$name did not exit yet. Close it from its own window.';
  }
}
