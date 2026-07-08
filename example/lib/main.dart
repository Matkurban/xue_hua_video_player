import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';
import 'package:xue_hua_video_player/xue_hua_video_player.dart';
import 'package:xue_hua_video_player_example/video_url.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await XueHuaVideoPlayer.initialize();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: '雪花视频播放器',
      theme: ThemeData(
        colorSchemeSeed: Colors.indigo,
        useMaterial3: true,
        extensions: [VideoControlsTheme.cupertino()],
      ),
      darkTheme: ThemeData(
        colorSchemeSeed: Colors.indigo,
        brightness: Brightness.dark,
        useMaterial3: true,
        extensions: [VideoControlsTheme.cupertino()],
      ),
      home: const PlayerPage(),
    );
  }
}

class PlayerPage extends StatefulWidget {
  const PlayerPage({super.key});

  @override
  State<PlayerPage> createState() => _PlayerPageState();
}

class _PlayerPageState extends State<PlayerPage> {
  final XueHuaPlayerController _controller = XueHuaPlayerController();
  final TextEditingController _urlController = TextEditingController(
    text:
        // 'https://flutter.github.io/assets-for-api-docs/assets/videos/butterfly.mp4',
        videoUrl,
  );
  bool _ready = false;
  bool _showControls = true;
  VideoControlsStyle _style = VideoControlsStyle.adaptive;
  String? _initError;

  @override
  void initState() {
    super.initState();
    _controller
        .initialize()
        .then((_) {
          debugPrint(
            'xue_hua_video_player: playerId=${_controller.playerId.value}',
          );
          if (mounted) setState(() => _ready = true);
        })
        .catchError((Object e, StackTrace st) {
          debugPrint('xue_hua_video_player initialize failed: $e\n$st');
          if (mounted) {
            setState(() {
              _initError = e.toString();
              _ready = true;
            });
          }
        });
  }

  @override
  void dispose() {
    _urlController.dispose();
    _controller.dispose();
    super.dispose();
  }

  Future<void> _open() async {
    FocusScope.of(context).unfocus();
    final text = _urlController.text.trim();
    final source = text.startsWith('/') || text.startsWith('file:')
        ? VideoSource.file(text)
        : VideoSource.network(text);
    await _controller.open(source, autoPlay: true);
  }

  Future<void> _openAsset() async {
    FocusScope.of(context).unfocus();
    await _controller.open(
      const VideoSource.asset('assets/sample.mp4'),
      autoPlay: true,
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('雪花视频播放器')),
      body: !_ready
          ? const Center(child: CircularProgressIndicator())
          : _initError != null
          ? Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: SelectableText(
                  '初始化失败: $_initError',
                  style: const TextStyle(color: Colors.red),
                ),
              ),
            )
          : Column(
              children: [
                Padding(
                  padding: const EdgeInsets.all(12),
                  child: Row(
                    children: [
                      Expanded(
                        child: TextField(
                          controller: _urlController,
                          decoration: const InputDecoration(
                            labelText: '视频地址（URL 或本地路径）',
                            border: OutlineInputBorder(),
                            isDense: true,
                          ),
                          onSubmitted: (_) => _open(),
                        ),
                      ),
                      const SizedBox(width: 8),
                      FilledButton.icon(
                        onPressed: _open,
                        icon: const Icon(Icons.play_circle_fill),
                        label: const Text('播放'),
                      ),
                      const SizedBox(width: 8),
                      OutlinedButton.icon(
                        onPressed: _openAsset,
                        icon: const Icon(Icons.folder_special),
                        label: const Text('资源'),
                      ),
                    ],
                  ),
                ),
                Expanded(
                  child: ColoredBox(
                    color: Colors.black,
                    child: SizedBox.expand(
                      child: XueHuaVideoView(
                        controller: _controller,
                        showControls: _showControls,
                        controlsStyle: _style,
                      ),
                    ),
                  ),
                ),
                _buildOptions(),
              ],
            ),
    );
  }

  Widget _buildOptions() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            children: [
              const Text('内置控制器'),
              const Spacer(),
              Switch(
                value: _showControls,
                onChanged: (v) => setState(() => _showControls = v),
              ),
            ],
          ),
          const SizedBox(height: 8),
          SegmentedButton<VideoControlsStyle>(
            segments: const [
              ButtonSegment(
                value: VideoControlsStyle.adaptive,
                label: Text('自适应'),
              ),
              ButtonSegment(
                value: VideoControlsStyle.material,
                label: Text('Material'),
              ),
              ButtonSegment(
                value: VideoControlsStyle.cupertino,
                label: Text('Cupertino'),
              ),
            ],
            selected: {_style},
            onSelectionChanged: _showControls
                ? (s) => setState(() => _style = s.first)
                : null,
          ),
          const SizedBox(height: 8),
          SignalBuilder(
            builder: (context) {
              final error = _controller.error.value;
              if (error != null) {
                developer.log(error);
              }
              final buffering = _controller.bufferingPercent.value;
              return Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  // Reserve a fixed height so the transient buffering line does
                  // not reflow the layout (and resize the video) while seeking.
                  SizedBox(
                    height: 20,
                    child: buffering < 100
                        ? Center(child: Text('缓冲 $buffering%'))
                        : null,
                  ),
                  if (error != null)
                    SelectableText(
                      '错误: $error',
                      maxLines: 2,
                      style: const TextStyle(color: Colors.red),
                    ),
                ],
              );
            },
          ),
        ],
      ),
    );
  }
}
