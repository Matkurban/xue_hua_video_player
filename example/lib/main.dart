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

  bool _ready = false;

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
    _controller.dispose();
    super.dispose();
  }

  Future<void> _openNetwork() async {
    await _controller.open(VideoSource.network(videoUrl), autoPlay: true);
  }

  Future<void> _openAsset() async {
    await _controller.open(
      const VideoSource.asset('assets/sample.mp4'),
      autoPlay: true,
    );
  }

  @override
  Widget build(BuildContext context) {
    return SignalBuilder(
      builder: (context) {
        final isFullscreen = _controller.isFullscreen.value;
        return Scaffold(
          appBar: isFullscreen
              ? null
              : AppBar(
                  title: const Text('雪花视频播放器'),
                  toolbarHeight: 48,
                  actions: [
                    TextButton(
                      onPressed: _openNetwork,
                      style: TextButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      child: const Text('网络'),
                    ),
                    TextButton(
                      onPressed: _openAsset,
                      style: TextButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      child: const Text('本地'),
                    ),
                  ],
                ),
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
              : ColoredBox(
                  color: Colors.black,
                  child: XueHuaVideoView(
                    controller: _controller,
                    showControls: true,
                    controlsStyle: .material,
                  ),
                ),
        );
      },
    );
  }
}
