import 'dart:io';

import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';
import '../util/dto.dart';

class LibraryPage extends StatefulWidget {
  const LibraryPage({
    super.key,
    required this.trainers,
    required this.isRunning,
    required this.onLaunch,
    required this.onStop,
    required this.onEdit,
    required this.onRemove,
    required this.onReset,
  });

  final List<TrainerEntry> trainers;
  final bool Function(String id) isRunning;
  final Future<void> Function(TrainerEntry entry) onLaunch;
  final Future<void> Function(TrainerEntry entry) onStop;
  final void Function(TrainerEntry entry) onEdit;
  final void Function(TrainerEntry entry) onRemove;

  /// Resets FLiNG trainer data; offered once the trainer ran in a known prefix.
  final void Function(TrainerEntry entry) onReset;

  @override
  State<LibraryPage> createState() => _LibraryPageState();
}

class _LibraryPageState extends State<LibraryPage> {
  String _hoveredId = '';

  /// Entries with a launch or stop in progress.
  final Set<String> _busy = {};

  Future<void> _run(TrainerEntry entry, Future<void> Function(TrainerEntry entry) action) async {
    if (_busy.contains(entry.id)) return;
    setState(() => _busy.add(entry.id));
    try {
      await action(entry);
    } finally {
      if (mounted) setState(() => _busy.remove(entry.id));
    }
  }

  Future<void> _launch(TrainerEntry entry) => _run(entry, widget.onLaunch);

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

  Widget _runningBadge(AppLocalizations l10n) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(color: Colors.black.withValues(alpha: 0.65), borderRadius: BorderRadius.circular(10)),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.circle, size: 9, color: Color(0xFF3DFFE9)),
          const SizedBox(width: 5),
          Text(l10n.running, style: const TextStyle(fontSize: 12, color: Colors.white)),
        ],
      ),
    );
  }

  Widget _card(BuildContext context, TrainerEntry entry) {
    final l10n = AppLocalizations.of(context)!;
    final busy = _busy.contains(entry.id);
    final running = widget.isRunning(entry.id);
    final showOverlay = busy || _hoveredId == entry.id;
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
            child: Stack(
              children: [
                if (showOverlay)
                  Positioned.fill(
                    child: Column(children: [
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
                          style: ElevatedButton.styleFrom(backgroundColor: running ? const Color(0xFFD64545) : const Color(0xFF00D3C4)),
                          onPressed: busy ? null : () => running ? _run(entry, widget.onStop) : _launch(entry),
                          child: busy
                              ? const SizedBox(height: 22, width: 22, child: CircularProgressIndicator(color: Colors.white, strokeWidth: 2))
                              : Text(running ? l10n.stop : l10n.launch),
                        ),
                      ),
                      const SizedBox(height: 25),
                    ]),
                  ),
                if (running) Positioned(left: 8, bottom: showOverlay ? 75 : 8, child: _runningBadge(l10n)),
              ],
            ),
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
