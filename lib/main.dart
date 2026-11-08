import 'dart:io';

import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

import 'l10n/app_localizations.dart';
import 'page/library.dart';
import 'util/dto.dart';
import 'util/game_launcher.dart';
import 'util/language.dart';
import 'util/storage.dart';
import 'util/widget.dart';
import 'widget/custom_setting_dialog.dart';
import 'widget/trainer_dialog.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const MyApp());
}

class MyApp extends StatefulWidget {
  const MyApp({super.key});

  static void setLocal(BuildContext context, String local) {
    context.findAncestorStateOfType<_MyAppState>()?._setLocale(local);
  }

  @override
  State<MyApp> createState() => _MyAppState();
}

class _MyAppState extends State<MyApp> {
  Locale _locale = const Locale('en');

  void _setLocale(String locale) {
    setState(() {
      _locale = Locale(locale);
    });
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'KaKa Trainer Launcher',
      locale: _locale,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      theme: ThemeData(
        brightness: Brightness.dark,
        primaryColor: const Color(0xFF181E42),
        useMaterial3: false,
      ),
      home: const MyHomePage(),
    );
  }
}

class MyHomePage extends StatefulWidget {
  const MyHomePage({super.key});

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> {
  List<TrainerEntry> trainers = [];
  bool loading = true;
  bool _closing = false;
  Window? _window;
  ListenerId? _closeListener;

  @override
  void initState() {
    super.initState();
    // Every close request (title bar, compositor, Alt+F4) is cancelled and
    // replaced by KaKa's own shutdown: save, stop trainers, exit.
    _window = WindowManager.instance.getCurrent();
    _closeListener = _window?.addListener((event) {
      if (event is WindowCloseRequestedEvent) {
        event.request.cancel();
        _shutdown();
      }
    });
    _load();
  }

  @override
  void dispose() {
    final listener = _closeListener;
    if (listener != null) _window?.removeListener(listener);
    super.dispose();
  }

  Future<void> _load() async {
    final config = await localStorage.readConfig();
    final language = config.language;
    if (language != null && language != selectedLanguage && languageOptions.contains(language) && mounted) {
      updateLanguage(language);
    }
    customSteamPath = config.steamPath;
    final entries = await localStorage.readLibrary();
    if (!mounted) return;
    setState(() {
      trainers = entries;
      loading = false;
    });
  }

  void updateLanguage(String newLanguage) {
    selectedLanguage = newLanguage;
    MyApp.setLocal(context, languageToLocale(selectedLanguage));
  }

  Future<void> _saveLibrary() => localStorage.writeLibrary(trainers);

  Future<void> _saveConfig() => localStorage.writeConfig(language: selectedLanguage, steamPath: customSteamPath);

  Future<void> _addTrainer() async {
    final entry = await showDialog<TrainerEntry>(context: context, barrierDismissible: false, builder: (_) => const TrainerDialog());
    if (entry == null || !mounted) return;
    setState(() => trainers.insert(0, entry));
    _saveLibrary();
    showSnakeBar(context: context, message: AppLocalizations.of(context)!.trainerAdded);
  }

  Future<void> _editTrainer(TrainerEntry entry) async {
    final updated = await showDialog<TrainerEntry>(context: context, barrierDismissible: false, builder: (_) => TrainerDialog(initial: entry));
    if (updated == null || !mounted) return;
    setState(() {
      final index = trainers.indexWhere((e) => e.id == entry.id);
      if (index != -1) trainers[index] = updated;
    });
    _saveLibrary();
    showSnakeBar(context: context, message: AppLocalizations.of(context)!.trainerUpdated);
  }

  Future<void> _removeTrainer(TrainerEntry entry) async {
    final l10n = AppLocalizations.of(context)!;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        content: Text(l10n.removeTrainerConfirm(entry.name)),
        actions: [
          TextButton(onPressed: () => Navigator.of(context).pop(false), child: Text(l10n.cancel)),
          TextButton(onPressed: () => Navigator.of(context).pop(true), child: Text(l10n.remove)),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    setState(() => trainers.removeWhere((e) => e.id == entry.id));
    _saveLibrary();
  }

  Future<void> _launch(TrainerEntry entry) {
    setState(() {
      trainers.removeWhere((e) => e.id == entry.id);
      trainers.insert(0, entry);
    });
    _saveLibrary();
    return launchTrainer(context, entry);
  }

  Future<void> _shutdown() async {
    if (_closing) return;
    _closing = true;
    try {
      await _saveConfig();
      await _saveLibrary();
      await stopAllTrainers();
    } catch (e) {
      debugPrint('Shutdown cleanup failed: $e');
    } finally {
      // Leave directly instead of letting the window be destroyed: on X11 the
      // Flutter Linux embedder aborts while disposing its GL framebuffer during
      // window teardown (no current GL context; upstream LaLa issue #80). Always
      // exit, so a failed save cannot leave a window that ignores every close.
      exit(0);
    }
  }

  Widget _appBarButton(IconData icon, String tooltip, VoidCallback onPressed) {
    return IconButton(
      icon: Icon(icon),
      tooltip: tooltip,
      color: Colors.grey,
      splashColor: Colors.transparent,
      highlightColor: Colors.transparent,
      hoverColor: Colors.transparent,
      onPressed: onPressed,
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      backgroundColor: Theme.of(context).primaryColor,
      appBar: AppBar(
        backgroundColor: Theme.of(context).primaryColor,
        elevation: 0,
        leading: Padding(padding: const EdgeInsets.all(12), child: Image.asset('image/kaka.png', filterQuality: FilterQuality.medium)),
        title: Text(l10n.title),
        actions: [
          _appBarButton(Icons.add, l10n.addTrainer, _addTrainer),
          _appBarButton(Icons.settings, l10n.setting, () {
            showDialog(
              context: context,
              barrierDismissible: false,
              builder: (BuildContext context) => CustomSettingDialog(updateLanguage: updateLanguage, onConfigChanged: _saveConfig),
            );
          }),
          _appBarButton(Icons.exit_to_app, MaterialLocalizations.of(context).closeButtonTooltip, _shutdown),
          const SizedBox(width: 10),
        ],
      ),
      body: loading
          ? const Center(child: CircularProgressIndicator())
          : LibraryPage(trainers: trainers, onLaunch: _launch, onEdit: _editTrainer, onRemove: _removeTrainer),
    );
  }
}
