// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for German (`de`).
class AppLocalizationsDe extends AppLocalizations {
  AppLocalizationsDe([String locale = 'de']) : super(locale);

  @override
  String get title => 'KaKa Trainer Starter';

  @override
  String get launch => 'Starten';

  @override
  String get libraryEmpty => 'Noch keine Trainer.\nKlicke auf +, um einen Trainer (.exe) für eines deiner Spiele hinzuzufügen.';

  @override
  String get addTrainer => 'Trainer hinzufügen';

  @override
  String get editTrainer => 'Trainer bearbeiten';

  @override
  String get edit => 'Bearbeiten';

  @override
  String get gameName => 'Spielname';

  @override
  String get trainerFile => 'Trainer (.exe)';

  @override
  String get gameCover => 'Coverbild (optional)';

  @override
  String get steamAppId => 'Steam-AppID (optional)';

  @override
  String get steamAppIdHelp => 'Nur nötig, wenn mehrere Spiele gleichzeitig laufen: wählt das richtige aus.';

  @override
  String get select => 'Auswählen';

  @override
  String get change => 'Ändern';

  @override
  String get save => 'Speichern';

  @override
  String get cancel => 'Abbrechen';

  @override
  String get remove => 'Entfernen';

  @override
  String removeTrainerConfirm(Object name) {
    return '\"$name\" aus der Bibliothek entfernen?\n\nDie Trainer-Datei selbst wird nicht gelöscht.';
  }

  @override
  String get trainerAdded => 'Trainer hinzugefügt';

  @override
  String get trainerUpdated => 'Trainer aktualisiert';

  @override
  String get requiredFields => 'Spielname und Trainer-Datei sind erforderlich.';

  @override
  String get invalidAppId => 'Die Steam-AppID muss eine positive Zahl sein.';

  @override
  String get setting => 'Einstellungen';

  @override
  String get language => 'Sprache';

  @override
  String get steamPath => 'Steam-Pfad';

  @override
  String get steamPathAuto => 'Automatisch';

  @override
  String get set => 'Festlegen';

  @override
  String get reset => 'Zurücksetzen';

  @override
  String get steamPathSet => 'Steam-Pfad festgelegt';

  @override
  String get ok => 'OK';

  @override
  String get launchFail => 'Start fehlgeschlagen';

  @override
  String launched(Object name) {
    return 'Trainer gestartet: $name';
  }

  @override
  String trainerPathNotFound(Object trainerPath) {
    return 'Trainer-Datei nicht gefunden.\n\nPfad: $trainerPath';
  }

  @override
  String get gameNotRunning =>
      'Kein laufendes Proton-Spiel gefunden.\n\nStarte zuerst das Spiel (Steam, Heroic, Lutris oder ein anderer Proton-Launcher) und dann den Trainer.';

  @override
  String multipleGamesRunning(Object games) {
    return 'Mehrere Proton-Spiele laufen:\n\n$games\n\nLege die Steam-AppID für diesen Trainer fest, um eines auszuwählen.';
  }

  @override
  String get gameStarting => 'Das Spiel startet noch (das Proton-Präfix ist noch nicht bereit).\n\nVersuche es in ein paar Sekunden erneut.';

  @override
  String protonPathNotFound(Object prefix) {
    return 'Proton-Installation für dieses Spiel nicht gefunden.\n\nPräfix: $prefix';
  }

  @override
  String trainerExited(Object code, Object logPath, Object output) {
    return 'Der Trainer wurde sofort beendet (Exit-Code $code).\n\n$output\n\nVollständiges Protokoll: $logPath';
  }

  @override
  String get sourceCode => 'Quellcode';

  @override
  String get basedOn => 'Basiert auf LaLa';

  @override
  String get openLog => 'Protokoll öffnen';

  @override
  String get trainerLogs => 'Trainer-Protokolle';

  @override
  String get openFolder => 'Ordner öffnen';

  @override
  String get resetTrainerData => 'Trainer-Daten zurücksetzen';

  @override
  String resetTrainerDataConfirm(Object paths) {
    return 'Für FLiNG-Trainer, die \"previous trainer instance did not exit properly\" melden.\n\nSchließe zuerst Spiel und Trainer. KaKa löscht dann den Ordner FLiNGTrainer und leert den Temp-Ordner im Präfix des Spiels:\n\n$paths\n\nSonst wird nichts verändert.';
  }

  @override
  String resetTrainerDataDone(Object count) {
    return 'Trainer-Daten zurückgesetzt ($count Einträge entfernt)';
  }

  @override
  String get prefixInUse => 'Im Präfix laufen noch das Spiel oder ein Trainer. Schließe sie und versuche es erneut.';

  @override
  String get cleanupOutsidePrefix => 'Nicht zurückgesetzt: Der AppData-Ordner dieses Präfixes zeigt aus dem Präfix heraus.';

  @override
  String get running => 'Läuft';

  @override
  String get stop => 'Beenden';

  @override
  String trainerAlreadyRunning(Object name) {
    return '$name läuft bereits.';
  }

  @override
  String trainerStopped(Object name) {
    return 'Trainer beendet: $name';
  }

  @override
  String trainerStopFailed(Object name) {
    return '$name wurde noch nicht beendet. Schließe ihn über sein eigenes Fenster.';
  }
}
