import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_de.dart';
import 'app_localizations_en.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale) : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate = _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates = <LocalizationsDelegate<dynamic>>[
    delegate,
    GlobalMaterialLocalizations.delegate,
    GlobalCupertinoLocalizations.delegate,
    GlobalWidgetsLocalizations.delegate,
  ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[Locale('de'), Locale('en'), Locale('zh')];

  /// No description provided for @title.
  ///
  /// In en, this message translates to:
  /// **'KaKa Trainer Launcher'**
  String get title;

  /// No description provided for @launch.
  ///
  /// In en, this message translates to:
  /// **'Launch'**
  String get launch;

  /// No description provided for @libraryEmpty.
  ///
  /// In en, this message translates to:
  /// **'No trainers yet.\nClick + to add a trainer (.exe) for one of your games.'**
  String get libraryEmpty;

  /// No description provided for @addTrainer.
  ///
  /// In en, this message translates to:
  /// **'Add Trainer'**
  String get addTrainer;

  /// No description provided for @editTrainer.
  ///
  /// In en, this message translates to:
  /// **'Edit Trainer'**
  String get editTrainer;

  /// No description provided for @edit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get edit;

  /// No description provided for @gameName.
  ///
  /// In en, this message translates to:
  /// **'Game name'**
  String get gameName;

  /// No description provided for @trainerFile.
  ///
  /// In en, this message translates to:
  /// **'Trainer (.exe)'**
  String get trainerFile;

  /// No description provided for @gameCover.
  ///
  /// In en, this message translates to:
  /// **'Cover image (optional)'**
  String get gameCover;

  /// No description provided for @steamAppId.
  ///
  /// In en, this message translates to:
  /// **'Steam AppID (optional)'**
  String get steamAppId;

  /// No description provided for @steamAppIdHelp.
  ///
  /// In en, this message translates to:
  /// **'Only needed when several games run at once: picks the right one.'**
  String get steamAppIdHelp;

  /// No description provided for @select.
  ///
  /// In en, this message translates to:
  /// **'Select'**
  String get select;

  /// No description provided for @change.
  ///
  /// In en, this message translates to:
  /// **'Change'**
  String get change;

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @remove.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get remove;

  /// No description provided for @removeTrainerConfirm.
  ///
  /// In en, this message translates to:
  /// **'Remove \"{name}\" from the library?\n\nThe trainer file itself is not deleted.'**
  String removeTrainerConfirm(Object name);

  /// No description provided for @trainerAdded.
  ///
  /// In en, this message translates to:
  /// **'Trainer added'**
  String get trainerAdded;

  /// No description provided for @trainerUpdated.
  ///
  /// In en, this message translates to:
  /// **'Trainer updated'**
  String get trainerUpdated;

  /// No description provided for @requiredFields.
  ///
  /// In en, this message translates to:
  /// **'Game name and trainer file are required.'**
  String get requiredFields;

  /// No description provided for @invalidAppId.
  ///
  /// In en, this message translates to:
  /// **'Steam AppID must be a positive number.'**
  String get invalidAppId;

  /// No description provided for @setting.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get setting;

  /// No description provided for @language.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get language;

  /// No description provided for @steamPath.
  ///
  /// In en, this message translates to:
  /// **'Steam path'**
  String get steamPath;

  /// No description provided for @steamPathAuto.
  ///
  /// In en, this message translates to:
  /// **'Auto-detect'**
  String get steamPathAuto;

  /// No description provided for @set.
  ///
  /// In en, this message translates to:
  /// **'Set'**
  String get set;

  /// No description provided for @reset.
  ///
  /// In en, this message translates to:
  /// **'Reset'**
  String get reset;

  /// No description provided for @steamPathSet.
  ///
  /// In en, this message translates to:
  /// **'Steam path set'**
  String get steamPathSet;

  /// No description provided for @ok.
  ///
  /// In en, this message translates to:
  /// **'OK'**
  String get ok;

  /// No description provided for @launchFail.
  ///
  /// In en, this message translates to:
  /// **'Launch failed'**
  String get launchFail;

  /// No description provided for @launched.
  ///
  /// In en, this message translates to:
  /// **'Trainer started: {name}'**
  String launched(Object name);

  /// No description provided for @trainerPathNotFound.
  ///
  /// In en, this message translates to:
  /// **'Trainer file not found.\n\nPath: {trainerPath}'**
  String trainerPathNotFound(Object trainerPath);

  /// No description provided for @gameNotRunning.
  ///
  /// In en, this message translates to:
  /// **'No running Proton game found.\n\nStart the game first (Steam, Heroic, Lutris or another Proton launcher), then launch the trainer.'**
  String get gameNotRunning;

  /// No description provided for @multipleGamesRunning.
  ///
  /// In en, this message translates to:
  /// **'Several Proton games are running:\n\n{games}\n\nSet the Steam AppID on this trainer to choose one.'**
  String multipleGamesRunning(Object games);

  /// No description provided for @gameStarting.
  ///
  /// In en, this message translates to:
  /// **'The game is still starting (its Proton prefix is not ready yet).\n\nTry again in a few seconds.'**
  String get gameStarting;

  /// No description provided for @protonPathNotFound.
  ///
  /// In en, this message translates to:
  /// **'Proton installation for this game not found.\n\nPrefix: {prefix}'**
  String protonPathNotFound(Object prefix);

  /// No description provided for @trainerExited.
  ///
  /// In en, this message translates to:
  /// **'The trainer exited right away (exit code {code}).\n\n{output}\n\nFull log: {logPath}'**
  String trainerExited(Object code, Object logPath, Object output);

  /// No description provided for @sourceCode.
  ///
  /// In en, this message translates to:
  /// **'Source'**
  String get sourceCode;

  /// No description provided for @basedOn.
  ///
  /// In en, this message translates to:
  /// **'Based on LaLa'**
  String get basedOn;
}

class _AppLocalizationsDelegate extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) => <String>['de', 'en', 'zh'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'de':
      return AppLocalizationsDe();
    case 'en':
      return AppLocalizationsEn();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError('AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
      'an issue with the localizations generation tool. Please file an issue '
      'on GitHub with a reproducible sample app and the gen-l10n configuration '
      'that was used.');
}
