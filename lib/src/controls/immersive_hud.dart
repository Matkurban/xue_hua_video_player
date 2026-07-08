import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';

import '../utils/platform_util.dart';
import 'immersive_controls_state.dart';

/// 按 HUD 类型与平台返回对齐方式 / Alignment for HUD kind and platform.
Alignment hudAlignmentFor(ImmersiveHudSnapshot snap) {
  if (isMobilePlatform) return Alignment.center;
  return switch (snap.kind) {
    ImmersiveHudKind.seek => const Alignment(0, -0.35),
    ImmersiveHudKind.volume => const Alignment(-0.82, 0),
    ImmersiveHudKind.brightness => const Alignment(0.82, 0),
    ImmersiveHudKind.playPause => Alignment.center,
  };
}

/// 沉浸操作瞬时 HUD / Transient HUD for immersive seek, brightness, and volume.
class ImmersiveHud extends StatelessWidget {
  /// 创建 HUD overlay / Creates the HUD overlay.
  const ImmersiveHud({super.key, required this.immersive});

  /// 沉浸 signals / Immersive signals.
  final ImmersiveControlsState immersive;

  @override
  Widget build(BuildContext context) {
    return IgnorePointer(
      child: SignalBuilder(
        builder: (context) {
          final snap = immersive.hud.value;
          return AnimatedOpacity(
            opacity: snap == null ? 0 : 1,
            duration: const Duration(milliseconds: 150),
            child: snap == null
                ? const SizedBox.shrink()
                : Align(
                    alignment: hudAlignmentFor(snap),
                    child: _HudContent(snapshot: snap),
                  ),
          );
        },
      ),
    );
  }
}

class _HudContent extends StatelessWidget {
  const _HudContent({required this.snapshot});

  final ImmersiveHudSnapshot snapshot;

  @override
  Widget build(BuildContext context) {
    final (IconData icon, String? text) = switch (snapshot.kind) {
      ImmersiveHudKind.seek => (
        snapshot.forward ? Icons.forward_10 : Icons.replay_10,
        '${snapshot.value.round()}s',
      ),
      ImmersiveHudKind.brightness => (
        Icons.brightness_6,
        '${(snapshot.value * 100).round()}%',
      ),
      ImmersiveHudKind.volume => (
        snapshot.value == 0 ? Icons.volume_off : Icons.volume_up,
        '${(snapshot.value * 100).round()}%',
      ),
      ImmersiveHudKind.playPause => (
        snapshot.value >= 0.5 ? Icons.pause : Icons.play_arrow,
        null,
      ),
    };

    return DecoratedBox(
      decoration: BoxDecoration(
        color: Colors.black54,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, color: Colors.white, size: 36),
            if (text != null) ...[
              const SizedBox(height: 6),
              Text(
                text,
                style: const TextStyle(color: Colors.white, fontSize: 14),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
