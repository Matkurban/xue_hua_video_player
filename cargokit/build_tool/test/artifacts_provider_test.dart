import 'dart:async';
import 'dart:io';

import 'package:build_tool/src/artifacts_provider.dart';
import 'package:http/http.dart';
import 'package:test/test.dart';

void main() {
  group('isRetriableDownloadError', () {
    test('retries common transient network failures', () {
      expect(isRetriableDownloadError(const SocketException('reset')), isTrue);
      expect(
        isRetriableDownloadError(
          ClientException('Connection closed while receiving data'),
        ),
        isTrue,
      );
      expect(isRetriableDownloadError(const HttpException('bad')), isTrue);
      expect(
        isRetriableDownloadError(TimeoutException('timed out')),
        isTrue,
      );
      expect(
        isRetriableDownloadError(const HandshakeException('tls')),
        isTrue,
      );
    });

    test('does not retry unrelated errors', () {
      expect(isRetriableDownloadError(Exception('logic error')), isFalse);
      expect(isRetriableDownloadError(ArgumentError('bad arg')), isFalse);
    });
  });

  group('downloadRetryDelay', () {
    test('uses exponential backoff capped at 30 seconds', () {
      expect(downloadRetryDelay(0), const Duration(seconds: 1));
      expect(downloadRetryDelay(1), const Duration(seconds: 2));
      expect(downloadRetryDelay(2), const Duration(seconds: 4));
      expect(downloadRetryDelay(4), const Duration(seconds: 16));
      expect(downloadRetryDelay(5), const Duration(seconds: 30));
      expect(downloadRetryDelay(10), const Duration(seconds: 30));
    });
  });

  group('retryDownloadRequest', () {
    test('retries ClientException and eventually succeeds', () async {
      var attempts = 0;
      final result = await retryDownloadRequest(
        Uri.parse('https://example.com/binary'),
        () async {
          attempts++;
          if (attempts < 3) {
            throw ClientException('Connection closed while receiving data');
          }
          return 'ok';
        },
        delayForAttempt: (_) => Duration.zero,
      );

      expect(result, 'ok');
      expect(attempts, 3);
    });

    test('stops after maxDownloadAttempts', () async {
      var attempts = 0;
      await expectLater(
        retryDownloadRequest(
          Uri.parse('https://example.com/binary'),
          () async {
            attempts++;
            throw ClientException('Connection closed while receiving data');
          },
          maxAttempts: 3,
          delayForAttempt: (_) => Duration.zero,
        ),
        throwsA(isA<ClientException>()),
      );
      expect(attempts, 3);
    });

    test('does not retry non-retriable errors', () async {
      var attempts = 0;
      await expectLater(
        retryDownloadRequest(
          Uri.parse('https://example.com/binary'),
          () async {
            attempts++;
            throw Exception('fatal');
          },
          delayForAttempt: (_) => Duration.zero,
        ),
        throwsA(isA<Exception>()),
      );
      expect(attempts, 1);
    });
  });

  group('downloadArtifactToFile', () {
    test('streams response body to disk', () async {
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      addTearDown(server.close);

      final payload = List<int>.generate(256 * 1024, (index) => index % 251);
      server.listen((request) async {
        request.response
          ..statusCode = HttpStatus.ok
          ..add(payload)
          ..close();
      });

      final tempDir = Directory.systemTemp.createTempSync('artifacts_provider_');
      addTearDown(() {
        if (tempDir.existsSync()) {
          tempDir.deleteSync(recursive: true);
        }
      });
      final destination = '${tempDir.path}/artifact.bin';

      await downloadArtifactToFile(
        Uri.parse('http://127.0.0.1:${server.port}/artifact'),
        destination,
      );

      expect(File(destination).readAsBytesSync(), payload);
    });
  });
}
