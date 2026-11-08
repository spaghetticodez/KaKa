import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:url_launcher/url_launcher.dart';

import '../l10n/app_localizations.dart';
import '../util/game_launcher.dart';
import '../util/language.dart';
import '../util/widget.dart';

const String _sourceUrl = 'https://github.com/spaghetticodez/KaKa';
const String _upstreamUrl = 'https://github.com/wyyadd/LaLa';

class CustomSettingDialog extends StatefulWidget {
  const CustomSettingDialog({super.key, required this.updateLanguage, required this.onConfigChanged});

  final ValueChanged<String> updateLanguage;
  final Future<void> Function() onConfigChanged;

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
    setState(() => customSteamPath = selected);
    await widget.onConfigChanged();
    if (!mounted) return;
    showSnakeBar(context: context, message: l10n.steamPathSet);
  }

  Future<void> _resetSteamPath() async {
    setState(() => customSteamPath = '');
    await widget.onConfigChanged();
  }

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
    final l10n = AppLocalizations.of(context)!;
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
                      value: selectedLanguage,
                      dropdownColor: const Color(0xFF2E3466),
                      focusColor: Colors.transparent,
                      onChanged: (String? newValue) {
                        if (newValue != null && newValue != selectedLanguage) {
                          setState(() => widget.updateLanguage(newValue));
                          widget.onConfigChanged();
                        }
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
                          customSteamPath.isEmpty ? l10n.steamPathAuto : customSteamPath,
                          style: const TextStyle(color: Colors.grey, fontSize: 13),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ],
                    ),
                  ),
                  if (customSteamPath.isNotEmpty) TextButton(onPressed: _resetSteamPath, child: Text(l10n.reset)),
                  ElevatedButton(
                    style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF00D3C4)),
                    onPressed: _pickSteamPath,
                    child: Text(l10n.set),
                  ),
                ],
              ),
              const SizedBox(height: 28),
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Text('KaKa $appVersion', style: const TextStyle(color: Colors.grey)),
                  const SizedBox(width: 12),
                  _link(l10n.sourceCode, _sourceUrl),
                  const SizedBox(width: 12),
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
