import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

import 'l10n/app_localizations.dart';
import 'page/library.dart';
import 'state/library_controller.dart';
import 'util/dto.dart';
import 'util/game_launcher.dart';
import 'util/language.dart';
import 'util/storage.dart';
import 'util/trainer_data.dart';
import 'util/widget.dart';
import 'widget/custom_setting_dialog.dart';
import 'widget/launch_error_dialog.dart';
import 'widget/trainer_dialog.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final controller = LibraryController(storage: LocalStorage(), launcher: TrainerLauncher());
  var closing = false;

  Future<void> quit() async {
    if (closing) return;
    closing = true;
    try {
      await controller.shutdown();
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

  // Every close request (title bar, compositor, Alt+F4) is cancelled and
  // replaced by KaKa's own shutdown: save, stop trainers, exit.
  WindowManager.instance.getCurrent()?.addListener((event) {
    if (event is WindowCloseRequestedEvent) {
      event.request.cancel();
      quit();
    }
  });

  runApp(KaKaApp(controller: controller, onQuit: quit));
  unawaited(controller.load());
}

class KaKaApp extends StatelessWidget {
  const KaKaApp({super.key, required this.controller, required this.onQuit});

  final LibraryController controller;
  final Future<void> Function() onQuit;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) => MaterialApp(
        debugShowCheckedModeBanner: false,
        title: 'KaKa Trainer Launcher',
        locale: Locale(languageToLocale(controller.language)),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        theme: ThemeData(
          brightness: Brightness.dark,
          primaryColor: const Color(0xFF181E42),
          useMaterial3: false,
        ),
        home: HomePage(controller: controller, onQuit: onQuit),
      ),
    );
  }
}

class HomePage extends StatelessWidget {
  const HomePage({super.key, required this.controller, required this.onQuit});

  final LibraryController controller;
  final Future<void> Function() onQuit;

  Future<void> _addTrainer(BuildContext context) async {
    final entry = await showDialog<TrainerEntry>(context: context, barrierDismissible: false, builder: (_) => const TrainerDialog());
    if (entry == null || !context.mounted) return;
    showSnakeBar(context: context, message: AppLocalizations.of(context)!.trainerAdded);
    await controller.add(entry);
  }

  Future<void> _editTrainer(BuildContext context, TrainerEntry entry) async {
    final updated = await showDialog<TrainerEntry>(context: context, barrierDismissible: false, builder: (_) => TrainerDialog(initial: entry));
    if (updated == null || !context.mounted) return;
    showSnakeBar(context: context, message: AppLocalizations.of(context)!.trainerUpdated);
    await controller.update(updated);
  }

  Future<void> _removeTrainer(BuildContext context, TrainerEntry entry) async {
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
    if (confirmed == true) await controller.remove(entry.id);
  }

  Future<void> _resetTrainerData(BuildContext context, TrainerEntry entry) async {
    final l10n = AppLocalizations.of(context)!;
    final cleanup = TrainerDataCleanup(entry.lastPrefix);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l10n.resetTrainerData),
        content: SingleChildScrollView(child: SelectableText(l10n.resetTrainerDataConfirm(cleanup.targets.join('\n')))),
        actions: [
          TextButton(onPressed: () => Navigator.of(context).pop(false), child: Text(l10n.cancel)),
          TextButton(onPressed: () => Navigator.of(context).pop(true), child: Text(l10n.reset)),
        ],
      ),
    );
    if (confirmed != true || !context.mounted) return;

    String message;
    try {
      message = l10n.resetTrainerDataDone(controller.resetTrainerData(entry));
    } on CleanupRefused catch (e) {
      message = e.reason == CleanupRefusal.prefixInUse ? l10n.prefixInUse : l10n.cleanupOutsidePrefix;
    } on FileSystemException catch (e) {
      message = e.toString();
    }
    showSnakeBar(context: context, message: message, time: 4);
  }

  Future<void> _launch(BuildContext context, TrainerEntry entry) async {
    final l10n = AppLocalizations.of(context)!;
    final error = await controller.launch(entry, l10n);
    if (!context.mounted) return;
    if (error == null) {
      showSnakeBar(context: context, message: l10n.launched(entry.name), time: 2);
    } else {
      // Not awaited: the card must leave its "launching" state while the dialog is open.
      unawaited(showLaunchError(context, error, openLog: controller.openLog));
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
          _appBarButton(Icons.add, l10n.addTrainer, () => _addTrainer(context)),
          _appBarButton(Icons.settings, l10n.setting, () {
            showDialog<void>(
              context: context,
              barrierDismissible: false,
              builder: (BuildContext context) => CustomSettingDialog(controller: controller),
            );
          }),
          _appBarButton(Icons.exit_to_app, MaterialLocalizations.of(context).closeButtonTooltip, onQuit),
          const SizedBox(width: 10),
        ],
      ),
      body: ListenableBuilder(
        listenable: controller,
        builder: (context, _) => controller.loading
            ? const Center(child: CircularProgressIndicator())
            : LibraryPage(
                trainers: controller.trainers,
                onLaunch: (entry) => _launch(context, entry),
                onEdit: (entry) => _editTrainer(context, entry),
                onRemove: (entry) => _removeTrainer(context, entry),
                onReset: (entry) => _resetTrainerData(context, entry),
              ),
      ),
    );
  }
}
