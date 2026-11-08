import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../util/game_launcher.dart';

/// Shows why a trainer launch failed, with a button to open the launch log
/// when [error] has one.
Future<void> showLaunchError(BuildContext context, LaunchException error, {required Future<bool> Function(String path) openLog}) {
  final l10n = AppLocalizations.of(context)!;
  final logPath = error.logPath;
  return showDialog<void>(
    context: context,
    builder: (BuildContext context) {
      return AlertDialog(
        title: Text(l10n.launchFail),
        content: SingleChildScrollView(child: SelectableText(error.message)),
        actions: <Widget>[
          if (logPath != null)
            TextButton(
              child: Text(l10n.openLog),
              onPressed: () => openLog(logPath),
            ),
          TextButton(
            child: Text(l10n.ok),
            onPressed: () => Navigator.of(context).pop(),
          ),
        ],
      );
    },
  );
}
