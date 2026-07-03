import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';
import 'package:xue_hua_video_player/xue_hua_video_player.dart';

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
        extensions: [VideoControlsTheme.material()],
      ),
      darkTheme: ThemeData(
        colorSchemeSeed: Colors.indigo,
        brightness: Brightness.dark,
        useMaterial3: true,
        extensions: [VideoControlsTheme.material()],
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
        'https://jsontodart.cn/api/object/7976982000/msg_video_7976982000_1782918277290246.mp4',
  );
  bool _ready = false;
  bool _showControls = true;
  VideoControlsStyle _style = VideoControlsStyle.adaptive;

  @override
  void initState() {
    super.initState();
    _controller.initialize().then((_) {
       setState(() => _ready = true);
    }).catchError((e) {
      debugPrint(e.toString());
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
    await _controller.open(
      VideoSource.network(_urlController.text),
      autoPlay: true,
    );
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
              debugPrint(error.toString());
              final buffering = _controller.bufferingPercent.value;
              return Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (buffering < 100) Text('缓冲 $buffering%'),
                  if (error != null)
                    SelectableText(
                      '错误: $error',
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
