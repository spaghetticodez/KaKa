import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';

/// Shows why a trainer launch failed.
Future<void> showLaunchError(BuildContext context, String message) {
  final l10n = AppLocalizations.of(context)!;
  return showDialog<void>(
    context: context,
    builder: (BuildContext context) {
      return AlertDialog(
        title: Text(l10n.launchFail),
        content: SingleChildScrollView(child: SelectableText(message)),
        actions: <Widget>[
          TextButton(
            child: Text(l10n.ok),
            onPressed: () => Navigator.of(context).pop(),
          ),
        ],
      );
    },
  );
}
