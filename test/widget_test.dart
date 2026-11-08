import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/l10n/app_localizations.dart';
import 'package:kaka/main.dart';
import 'package:kaka/state/library_controller.dart';
import 'package:kaka/util/game_launcher.dart';
import 'package:kaka/util/storage.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path/path.dart' as p;

import 'fakes.dart';

void main() {
  final l10n = lookupAppLocalizations(const Locale('en'));
  late Directory tmp;
  late String exe;
  late MemoryStorage storage;
  late FakeLauncher launcher;
  late LibraryController controller;
  late List<Uri> opened;

  setUp(() {
    tmp = Directory.systemTemp.createTempSync('kaka_widget_');
    exe = (File(p.join(tmp.path, 'Trainer.exe'))..writeAsStringSync('MZ')).path;
    storage = MemoryStorage(library: [trainer('a', path: exe), trainer('b', path: exe)]);
    launcher = FakeLauncher(logDir: Directory(p.join(tmp.path, 'logs')));
    opened = [];
    controller = LibraryController(
      storage: storage,
      launcher: launcher,
      openUri: (uri) async {
        opened.add(uri);
        return true;
      },
    );
  });

  tearDown(() => tmp.deleteSync(recursive: true));

  Future<void> pumpApp(WidgetTester tester) async {
    tester.view.physicalSize = const Size(1280, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await controller.load();
    await tester.pumpWidget(KaKaApp(controller: controller, onQuit: () async {}));
    await tester.pumpAndSettle();
  }

  /// Card buttons only show while the mouse is over the card. One mouse per test.
  TestGesture? mouse;
  Future<void> hoverCard(WidgetTester tester, String id) async {
    var m = mouse;
    if (m == null) {
      m = mouse = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await m.addPointer(location: Offset.zero);
      addTearDown(() {
        mouse = null;
        return m!.removePointer();
      });
    }
    await m.moveTo(tester.getCenter(find.byKey(ValueKey('trainer-card-$id'))));
    await tester.pump();
  }

  /// Lets a snack bar run out so no timer is left pending. Its display timer
  /// only starts after the entrance animation, so settle first.
  Future<void> drainSnackBar(WidgetTester tester) async {
    await tester.pumpAndSettle();
    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();
  }

  testWidgets('an empty library shows the hint to add a trainer', (tester) async {
    storage.library = [];
    await pumpApp(tester);
    expect(find.text(l10n.libraryEmpty), findsOneWidget);
  });

  testWidgets('clicking a card launches it and confirms', (tester) async {
    await pumpApp(tester);
    await tester.tap(find.byKey(const ValueKey('trainer-card-b')));
    await tester.pump();
    expect(launcher.launched, ['b']);
    expect(controller.trainers.first.id, 'b', reason: 'last launched moves to the front');
    expect(find.text(l10n.launched('Game b')), findsOneWidget);
    await drainSnackBar(tester);
  });

  testWidgets('a failed launch shows the reason in an error dialog', (tester) async {
    launcher.failure = LaunchException(l10n.gameNotRunning);
    await pumpApp(tester);
    await tester.tap(find.byKey(const ValueKey('trainer-card-a')));
    await tester.pumpAndSettle();
    expect(find.text(l10n.launchFail), findsOneWidget);
    expect(find.text(l10n.gameNotRunning), findsOneWidget);

    await tester.tap(find.text(l10n.ok));
    await tester.pumpAndSettle();
    expect(find.text(l10n.launchFail), findsNothing);
    expect(find.text(l10n.openLog), findsNothing, reason: 'no log was written for this failure');
  });

  testWidgets('a trainer that crashed offers to open its launch log', (tester) async {
    launcher.failure = LaunchException('exited with code 3', logPath: '/logs/crash.log');
    await pumpApp(tester);
    await tester.tap(find.byKey(const ValueKey('trainer-card-a')));
    await tester.pumpAndSettle();
    await tester.tap(find.text(l10n.openLog));
    await tester.pump();
    expect(opened, [Uri.file('/logs/crash.log')]);
  });

  testWidgets('settings show the log folder and open it', (tester) async {
    PackageInfo.setMockInitialValues(appName: 'kaka', packageName: 'kaka', version: '1.2.3', buildNumber: '1', buildSignature: '');
    await pumpApp(tester);
    await tester.tap(find.byTooltip(l10n.setting));
    await tester.pumpAndSettle();
    expect(find.text(launcher.logDir.path), findsOneWidget);
    expect(find.text('KaKa v1.2.3'), findsOneWidget);

    await tester.tap(find.text(l10n.openFolder));
    await tester.pump();
    expect(opened, [Uri.directory(launcher.logDir.path)]);
    expect(launcher.logDir.existsSync(), isTrue);
  });

  testWidgets('reset trainer data: only offered after a launch, lists the paths, then cleans', (tester) async {
    final compat = p.join(tmp.path, 'compatdata', '7');
    final fling = Directory(p.join(compat, 'pfx', 'drive_c', 'users', 'steamuser', 'AppData', 'Local', 'FLiNGTrainer'))..createSync(recursive: true);
    storage.library = [trainer('a', path: exe, lastPrefix: compat), trainer('b', path: exe)];
    controller = LibraryController(storage: storage, launcher: launcher, processes: () => []);
    await pumpApp(tester);

    await hoverCard(tester, 'b');
    expect(find.byTooltip(l10n.resetTrainerData), findsNothing, reason: 'never launched: no known prefix');

    await hoverCard(tester, 'a');
    await tester.tap(find.byTooltip(l10n.resetTrainerData));
    await tester.pumpAndSettle();
    expect(find.textContaining(fling.path), findsOneWidget);
    expect(fling.existsSync(), isTrue, reason: 'nothing happens before confirming');

    await tester.tap(find.widgetWithText(TextButton, l10n.reset));
    await tester.pumpAndSettle();
    expect(fling.existsSync(), isFalse);
    expect(find.text(l10n.resetTrainerDataDone(1)), findsOneWidget);
    await drainSnackBar(tester);
  });

  testWidgets('a running trainer shows a badge and a Stop button, and is not launched twice', (tester) async {
    launcher.live.add('a');
    await pumpApp(tester);
    controller.refreshRunning();
    await tester.pump();
    expect(find.text(l10n.running), findsOneWidget, reason: 'badge is visible without hovering');

    await tester.tap(find.byKey(const ValueKey('trainer-card-a')));
    await tester.pump();
    expect(launcher.launched, isEmpty);
    expect(find.text(l10n.trainerAlreadyRunning('Game a')), findsOneWidget);
    await drainSnackBar(tester);

    await hoverCard(tester, 'a');
    await tester.tap(find.widgetWithText(ElevatedButton, l10n.stop));
    await tester.pumpAndSettle();
    expect(find.text(l10n.trainerStopped('Game a')), findsOneWidget);
    expect(find.text(l10n.running), findsNothing);
    expect(find.widgetWithText(ElevatedButton, l10n.launch), findsOneWidget);
    await drainSnackBar(tester);
  });

  testWidgets('remove asks first; cancel keeps the trainer', (tester) async {
    await pumpApp(tester);
    await hoverCard(tester, 'a');
    await tester.tap(find.byTooltip(l10n.remove));
    await tester.pumpAndSettle();
    expect(find.text(l10n.removeTrainerConfirm('Game a')), findsOneWidget);

    await tester.tap(find.widgetWithText(TextButton, l10n.cancel));
    await tester.pumpAndSettle();
    expect(storage.library, hasLength(2));

    await hoverCard(tester, 'a');
    await tester.tap(find.byTooltip(l10n.remove));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(TextButton, l10n.remove));
    await tester.pumpAndSettle();
    expect(storage.library.map((e) => e.id), ['b']);
    expect(find.text('Game a'), findsNothing);
  });

  testWidgets('edit validates input and saves the changed entry', (tester) async {
    await pumpApp(tester);
    await hoverCard(tester, 'a');
    await tester.tap(find.byTooltip(l10n.edit));
    await tester.pumpAndSettle();
    expect(find.text(l10n.editTrainer), findsOneWidget);

    final fields = find.byType(TextField);
    final save = find.widgetWithText(ElevatedButton, l10n.save);

    await tester.enterText(fields.first, '   ');
    await tester.tap(save);
    await tester.pump();
    expect(find.text(l10n.requiredFields), findsOneWidget);

    await tester.enterText(fields.first, 'Elden Ring');
    await tester.enterText(fields.last, '0');
    await tester.tap(save);
    await tester.pump();
    expect(find.text(l10n.invalidAppId), findsOneWidget);

    await tester.enterText(fields.last, '1245620');
    await tester.tap(save);
    await tester.pumpAndSettle();
    final saved = storage.library.firstWhere((e) => e.id == 'a');
    expect(saved.name, 'Elden Ring');
    expect(saved.steamAppId, 1245620);
    expect(find.text('Elden Ring'), findsOneWidget);
    await drainSnackBar(tester);
  });

  testWidgets('changing the language re-renders the app', (tester) async {
    storage.library = [];
    storage.config = const AppConfig(language: 'English');
    await pumpApp(tester);
    await controller.setLanguage('German');
    await tester.pumpAndSettle();
    expect(find.text(lookupAppLocalizations(const Locale('de')).libraryEmpty), findsOneWidget);
  });
}
