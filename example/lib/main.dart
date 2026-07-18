import 'package:chat_context_menu/chat_context_menu.dart';
import 'package:flutter/material.dart';
import 'package:signals/signals_flutter.dart';
import 'package:xue_hua_video_player/xue_hua_video_player.dart';
import 'dart:typed_data';

import 'thumbnail_page.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Start gst_init on a background thread; do not block the first frame.
  final pluginSw = Stopwatch()..start();
  await XueHuaVideoPlayer.initialize();
  runApp(const MyApp());
  debugPrint(
    '[xhvp-init-timing] example_plugin_init=${pluginSw.elapsedMilliseconds}ms',
  );
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: '雪花视频播放器',
      debugShowCheckedModeBanner: false,
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

  final List<String> mediaList = ThumbnailPage.networkSamples;

  @override
  void initState() {
    super.initState();
    final sw = Stopwatch()..start();
    _controller
        .initialize()
        .then((_) {
          debugPrint(
            '[xhvp-init-timing] example_controller_init='
            '${sw.elapsedMilliseconds}ms '
            'playerId=${_controller.playerId.value}',
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

  Future<void> _openAsset() async {
    await _controller.open(
      const VideoSource.asset('assets/sample.mp4'),
      autoPlay: true,
    );
  }

  Future<void> _showPng(Uint8List png, String title) async {
    if (!mounted) return;
    await showDialog<void>(
      context: context,
      builder: (context) {
        return AlertDialog(
          title: Text(title),
          content: Image.memory(png),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('关闭'),
            ),
          ],
        );
      },
    );
  }

  Future<void> _captureCurrentFrame() async {
    try {
      final png = await _controller.captureCurrentFrame();
      await _showPng(png, '当前帧');
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('截帧失败: $e')));
    }
  }

  void _openThumbnailPage() {
    Navigator.of(
      context,
    ).push(MaterialPageRoute<void>(builder: (_) => const ThumbnailPage()));
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
                    ChatContextMenuWrapper(
                      backgroundColor: Colors.white,
                      widgetBuilder: (context, shoeMenu, hideMenu) {
                        return TextButton(
                          onPressed: shoeMenu,
                          style: TextButton.styleFrom(
                            tapTargetSize: .shrinkWrap,
                            visualDensity: .compact,
                          ),
                          child: const Text('网络'),
                        );
                      },
                      menuBuilder: ((context, hideMenu) {
                        return Column(
                          mainAxisSize: .min,
                          crossAxisAlignment: .start,
                          children: List.generate(mediaList.length, (index) {
                            final media = mediaList[index];
                            return TextButton(
                              onPressed: () {
                                _controller.open(
                                  VideoSource.network(media),
                                  autoPlay: true,
                                );
                                hideMenu();
                              },
                              child: Text(media),
                            );
                          }),
                        );
                      }),
                    ),
                    TextButton(
                      onPressed: _openAsset,
                      style: TextButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      child: const Text('本地'),
                    ),
                    TextButton(
                      onPressed: _openThumbnailPage,
                      style: TextButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      child: const Text('抽封面'),
                    ),
                    TextButton(
                      onPressed: _captureCurrentFrame,
                      style: TextButton.styleFrom(
                        tapTargetSize: .shrinkWrap,
                        visualDensity: .compact,
                      ),
                      child: const Text('截帧'),
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
                    controlsStyle: .cupertino,
                  ),
                ),
        );
      },
    );
  }
}
