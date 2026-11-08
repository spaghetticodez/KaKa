import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:path/path.dart' as p;

import '../l10n/app_localizations.dart';
import '../util/dto.dart';

/// Add or edit a trainer. Pops with the resulting [TrainerEntry], or null on cancel.
class TrainerDialog extends StatefulWidget {
  const TrainerDialog({super.key, this.initial});

  final TrainerEntry? initial;

  @override
  State<TrainerDialog> createState() => _TrainerDialogState();
}

class _TrainerDialogState extends State<TrainerDialog> {
  late final TextEditingController _name = TextEditingController(text: widget.initial?.name ?? '');
  late final TextEditingController _appId = TextEditingController(text: widget.initial?.steamAppId?.toString() ?? '');
  late String? _trainerPath = widget.initial?.trainerPath;
  late String _coverPath = widget.initial?.coverImagePath ?? '';
  String? _error;

  @override
  void dispose() {
    _name.dispose();
    _appId.dispose();
    super.dispose();
  }

  Future<void> _pickTrainer() async {
    final file = await FilePicker.pickFile(type: FileType.custom, allowedExtensions: ['exe', 'EXE']);
    final path = file?.path;
    if (path == null) return;
    setState(() {
      _trainerPath = path;
      if (_name.text.trim().isEmpty) _name.text = p.basenameWithoutExtension(path);
    });
  }

  Future<void> _pickCover() async {
    final file = await FilePicker.pickFile(type: FileType.image);
    final path = file?.path;
    if (path != null) setState(() => _coverPath = path);
  }

  void _save() {
    final l10n = AppLocalizations.of(context)!;
    final name = _name.text.trim();
    final trainerPath = _trainerPath;
    if (name.isEmpty || trainerPath == null || trainerPath.isEmpty) {
      setState(() => _error = l10n.requiredFields);
      return;
    }
    int? appId;
    final appIdText = _appId.text.trim();
    if (appIdText.isNotEmpty) {
      appId = int.tryParse(appIdText);
      if (appId == null || appId <= 0) {
        setState(() => _error = l10n.invalidAppId);
        return;
      }
    }
    if (!File(trainerPath).existsSync()) {
      setState(() => _error = l10n.trainerPathNotFound(trainerPath));
      return;
    }
    Navigator.of(context).pop(TrainerEntry(
      id: widget.initial?.id ?? TrainerEntry.newId(),
      name: name,
      trainerPath: trainerPath,
      coverImagePath: _coverPath,
      steamAppId: appId,
    ));
  }

  Widget _fileRow(String label, String? path, String buttonText, VoidCallback onPick, {VoidCallback? onClear}) {
    return Row(
      children: [
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(label, style: const TextStyle(color: Colors.grey, fontSize: 13)),
              const SizedBox(height: 4),
              Tooltip(
                message: path ?? '',
                child: Text(path == null || path.isEmpty ? '—' : p.basename(path), overflow: TextOverflow.ellipsis),
              ),
            ],
          ),
        ),
        if (onClear != null && path != null && path.isNotEmpty) IconButton(icon: const Icon(Icons.clear, size: 18), onPressed: onClear),
        SizedBox(
          height: 36,
          width: 110,
          child: ElevatedButton(
            style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF00D3C4)),
            onPressed: onPick,
            child: Text(buttonText),
          ),
        ),
      ],
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
          padding: const EdgeInsets.fromLTRB(24, 12, 24, 20),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Text(widget.initial == null ? l10n.addTrainer : l10n.editTrainer, style: const TextStyle(color: Colors.grey)),
                  const Spacer(),
                  IconButton(icon: const Icon(Icons.close), color: Colors.grey, onPressed: () => Navigator.of(context).pop()),
                ],
              ),
              const SizedBox(height: 8),
              _fileRow(l10n.trainerFile, _trainerPath, _trainerPath == null ? l10n.select : l10n.change, _pickTrainer),
              const SizedBox(height: 20),
              TextField(
                controller: _name,
                maxLength: 80,
                decoration: InputDecoration(labelText: l10n.gameName, border: const OutlineInputBorder()),
              ),
              const SizedBox(height: 8),
              _fileRow(l10n.gameCover, _coverPath, _coverPath.isEmpty ? l10n.select : l10n.change, _pickCover,
                  onClear: () => setState(() => _coverPath = '')),
              const SizedBox(height: 20),
              TextField(
                controller: _appId,
                keyboardType: TextInputType.number,
                inputFormatters: [FilteringTextInputFormatter.digitsOnly],
                decoration: InputDecoration(
                  labelText: l10n.steamAppId,
                  helperText: l10n.steamAppIdHelp,
                  helperMaxLines: 3,
                  border: const OutlineInputBorder(),
                ),
              ),
              if (_error != null) ...[
                const SizedBox(height: 12),
                Text(_error!, style: const TextStyle(color: Colors.redAccent)),
              ],
              const SizedBox(height: 20),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(onPressed: () => Navigator.of(context).pop(), child: Text(l10n.cancel)),
                  const SizedBox(width: 10),
                  ElevatedButton(
                    style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF00D3C4)),
                    onPressed: _save,
                    child: Text(l10n.save),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
