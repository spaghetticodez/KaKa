import 'dart:io';

import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../util/dto.dart';

class LibraryPage extends StatefulWidget {
  const LibraryPage({super.key, required this.trainers, required this.onLaunch, required this.onEdit, required this.onRemove, required this.onReset});

  final List<TrainerEntry> trainers;
  final Future<void> Function(TrainerEntry entry) onLaunch;
  final void Function(TrainerEntry entry) onEdit;
  final void Function(TrainerEntry entry) onRemove;

  /// Resets FLiNG trainer data; offered once the trainer ran in a known prefix.
  final void Function(TrainerEntry entry) onReset;

  @override
  State<LibraryPage> createState() => _LibraryPageState();
}

class _LibraryPageState extends State<LibraryPage> {
  String _hoveredId = '';
  final Set<String> _launching = {};

  Future<void> _launch(TrainerEntry entry) async {
    if (_launching.contains(entry.id)) return;
    setState(() => _launching.add(entry.id));
    try {
      await widget.onLaunch(entry);
    } finally {
      if (mounted) setState(() => _launching.remove(entry.id));
    }
  }

  ImageProvider _cover(TrainerEntry entry) {
    if (entry.coverImagePath.isNotEmpty && File(entry.coverImagePath).existsSync()) {
      return FileImage(File(entry.coverImagePath));
    }
    return const AssetImage('image/default_game_cover.jpg');
  }

  Widget _iconButton(IconData icon, String tooltip, VoidCallback onPressed) {
    return IconButton(
      icon: Icon(icon),
      tooltip: tooltip,
      splashColor: Colors.transparent,
      highlightColor: Colors.transparent,
      hoverColor: Colors.transparent,
      onPressed: onPressed,
    );
  }

  Widget _card(BuildContext context, TrainerEntry entry) {
    final l10n = AppLocalizations.of(context)!;
    final launching = _launching.contains(entry.id);
    final showOverlay = launching || _hoveredId == entry.id;
    return Column(
      children: [
        InkWell(
          key: ValueKey('trainer-card-${entry.id}'),
          onTap: () => _launch(entry),
          onHover: (hovered) => setState(() => _hoveredId = hovered ? entry.id : ''),
          child: Container(
            height: 300,
            width: 200,
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(10),
              image: DecorationImage(image: _cover(entry), fit: BoxFit.cover),
            ),
            child: showOverlay
                ? Column(children: [
                    Container(
                      color: Colors.black.withValues(alpha: 0.3),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          if (entry.lastPrefix.isNotEmpty)
                            _iconButton(Icons.cleaning_services_outlined, l10n.resetTrainerData, () => widget.onReset(entry)),
                          _iconButton(Icons.edit_outlined, l10n.edit, () => widget.onEdit(entry)),
                          _iconButton(Icons.cancel_outlined, l10n.remove, () => widget.onRemove(entry)),
                        ],
                      ),
                    ),
                    const Spacer(),
                    SizedBox(
                      height: 40,
                      width: 160,
                      child: ElevatedButton(
                        style: ElevatedButton.styleFrom(backgroundColor: const Color(0xFF00D3C4)),
                        onPressed: launching ? null : () => _launch(entry),
                        child: launching
                            ? const SizedBox(height: 22, width: 22, child: CircularProgressIndicator(color: Colors.white, strokeWidth: 2))
                            : Text(l10n.launch),
                      ),
                    ),
                    const SizedBox(height: 25),
                  ])
                : null,
          ),
        ),
        Container(
          width: 220,
          padding: const EdgeInsets.symmetric(vertical: 10.0),
          child: Text(entry.name, textAlign: TextAlign.center),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    if (widget.trainers.isEmpty) {
      return Center(
        child: Text(
          AppLocalizations.of(context)!.libraryEmpty,
          style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 25),
          textAlign: TextAlign.center,
        ),
      );
    }
    return SingleChildScrollView(
      padding: const EdgeInsets.all(30),
      child: Wrap(
        spacing: 10,
        runSpacing: 20,
        children: [for (final entry in widget.trainers) _card(context, entry)],
      ),
    );
  }
}
