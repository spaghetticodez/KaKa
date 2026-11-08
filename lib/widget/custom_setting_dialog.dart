import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:url_launcher/url_launcher.dart';

import '../l10n/app_localizations.dart';
import '../state/library_controller.dart';
import '../util/language.dart';
import '../util/widget.dart';

const String _sourceUrl = 'https://github.com/spaghetticodez/KaKa';
const String _upstreamUrl = 'https://github.com/wyyadd/LaLa';

class CustomSettingDialog extends StatefulWidget {
  const CustomSettingDialog({super.key, required this.controller});

  final LibraryController controller;

  @override
  State<CustomSettingDialog> createState() => _CustomSettingDialogState();
}

class _CustomSettingDialogState extends State<CustomSettingDialog> {
  String appVersion = '';

  @override
  void initState() {
    super.initState();
    PackageInfo.fromPlatform().then((info) {
      if (mounted) setState(() => appVersion = 'v${info.version}');
    });
  }

  Future<void> _pickSteamPath() async {
    final l10n = AppLocalizations.of(context)!;
    final selected = await FilePicker.getDirectoryPath();
    if (selected == null) return;
    await widget.controller.setSteamPath(selected);
    if (!mounted) return;
    showSnakeBar(context: context, message: l10n.steamPathSet);
  }

  Future<void> _resetSteamPath() => widget.controller.setSteamPath('');

  Widget _link(String text, String url) {
    return InkWell(
      splashColor: Colors.transparent,
      highlightColor: Colors.transparent,
      hoverColor: Colors.transparent,
      onTap: () => launchUrl(Uri.parse(url)),
      child: Text(text, style: const TextStyle(color: Colors.grey, fontSize: 15, decoration: TextDecoration.underline)),
    );
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(listenable: widget.controller, builder: (context, _) => _build(context));
  }

  Widget _build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final steamPath = widget.controller.steamPath;
    return Dialog(
      backgroundColor: const Color(0xFF2E3466),
      child: SizedBox(
        width: 460,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 10, 20, 16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Text(l10n.setting, style: const TextStyle(color: Colors.grey)),
                  const Spacer(),
                  IconButton(icon: const Icon(Icons.close), color: Colors.grey, onPressed: () => Navigator.of(context).pop()),
                ],
              ),
              Row(
                children: [
                  Text(l10n.language, style: const TextStyle(fontSize: 18)),
                  const Spacer(),
                  SizedBox(
                    width: 140,
                    child: DropdownButton<String>(
                      isExpanded: true,
                      value: widget.controller.language,
                      dropdownColor: const Color(0xFF2E3466),
                      focusColor: Colors.transparent,
                      onChanged: (String? newValue) {
                        if (newValue != null) widget.controller.setLanguage(newValue);
                      },
                      items: [for (final value in languageOptions) DropdownMenuItem<String>(value: value, child: Center(child: Text(value)))],
                    ),
                  ),
                ],
              ),
              const Divider(height: 24),
              Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(l10n.steamPath, style: const TextStyle(fontSize: 18)),
                        const SizedBox(height: 4),
                        Text(
                          steamPath.isEmpty ? l10n.steamPathAuto : steamPath,
                          style: const TextStyle(color: Colors.grey, fontSize: 13),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ],
                    ),
                  ),
                  if (steamPath.isNotEmpty) TextButton(onPressed: _resetSteamPath, child: Text(l10n.reset)),
                  ElevatedButton(
                    style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF00D3C4)),
                    onPressed: _pickSteamPath,
                    child: Text(l10n.set),
                  ),
                ],
              ),
              const Divider(height: 24),
              Row(
                children: [
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(l10n.trainerLogs, style: const TextStyle(fontSize: 18)),
                        const SizedBox(height: 4),
                        Tooltip(
                          message: widget.controller.logDir,
                          child: Text(widget.controller.logDir,
                              style: const TextStyle(color: Colors.grey, fontSize: 13), overflow: TextOverflow.ellipsis),
                        ),
                      ],
                    ),
                  ),
                  ElevatedButton(
                    style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF00D3C4)),
                    onPressed: widget.controller.openLogFolder,
                    child: Text(l10n.openFolder),
                  ),
                ],
              ),
              const SizedBox(height: 28),
              // Wrap, not Row: long translations or large fonts must not overflow.
              Wrap(
                alignment: WrapAlignment.center,
                spacing: 12,
                runSpacing: 6,
                children: [
                  Text('KaKa $appVersion', style: const TextStyle(color: Colors.grey)),
                  _link(l10n.sourceCode, _sourceUrl),
                  _link(l10n.basedOn, _upstreamUrl),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
