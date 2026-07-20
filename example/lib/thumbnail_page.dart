import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:xue_hua_video_player/xue_hua_video_player.dart';
import 'package:xue_hua_video_player_example/video_url.dart';

/// Demo page: pick a video source, then call [XueHuaVideoPlayer.captureThumbnail].
class ThumbnailPage extends StatefulWidget {
  const ThumbnailPage({super.key});

  static const networkSamples = <String>[
    'https://media.w3.org/2010/05/bunny/trailer.mp4',
    'https://media.w3.org/2010/05/sintel/trailer.mp4',
    'https://media.w3.org/2010/05/video/movie_300.mp4',
    'https://www.w3schools.com/html/mov_bbb.mp4',
    'https://archive.org/download/big-bunny-sample-video/SampleVideo.mp4',
    'https://media.w3.org/2010/05/bunny/movie.mp4',
    videoUrl
  ];

  @override
  State<ThumbnailPage> createState() => _ThumbnailPageState();
}

class _ThumbnailPageState extends State<ThumbnailPage> {
  bool _busy = false;
  Uint8List? _png;
  String? _sourceLabel;

  Future<void> _capture(VideoSource source, String label) async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _png = null;
      _sourceLabel = label;
    });
    try {
      final png = await XueHuaVideoPlayer.captureThumbnail(source);
      if (!mounted) return;
      setState(() => _png = png);
    } catch (e, st) {
      debugPrint('抽封面失败 source=$label type=${source.type.name} error=$e\n$st');
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('抽封面失败: $e')));
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  Future<void> _pickLocalVideo() async {
    final result = await FilePicker.platform.pickFiles(type: FileType.video);
    final path = result?.files.single.path;
    if (path == null || path.isEmpty) return;
    await _capture(VideoSource.file(path), path);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('抽封面')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          FilledButton.icon(
            onPressed: _busy ? null : _pickLocalVideo,
            icon: const Icon(Icons.folder_open),
            label: const Text('选择本地视频'),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: _busy
                ? null
                : () => _capture(
                    const VideoSource.asset('assets/sample.mp4'),
                    'assets/sample.mp4',
                  ),
            icon: const Icon(Icons.video_file_outlined),
            label: const Text('内置 sample.mp4'),
          ),
          const SizedBox(height: 24),
          Text('网络示例', style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 8),
          ...ThumbnailPage.networkSamples.map((url) {
            return ListTile(
              contentPadding: EdgeInsets.zero,
              enabled: !_busy,
              title: Text(url, maxLines: 2, overflow: TextOverflow.ellipsis),
              trailing: const Icon(Icons.image_outlined),
              onTap: () => _capture(VideoSource.network(url), url),
            );
          }),
          const SizedBox(height: 24),
          if (_busy)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 24),
              child: Center(child: CircularProgressIndicator()),
            ),
          if (_sourceLabel != null) ...[
            Text('当前源', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            SelectableText(_sourceLabel!),
            const SizedBox(height: 16),
          ],
          if (_png != null) ...[
            Text('封面预览', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            ClipRRect(
              borderRadius: BorderRadius.circular(8),
              child: Image.memory(_png!),
            ),
          ],
        ],
      ),
    );
  }
}
