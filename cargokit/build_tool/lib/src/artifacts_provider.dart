/// This is copied from Cargokit (which is the official way to use it currently)
/// Details: https://fzyzcjy.github.io/flutter_rust_bridge/manual/integrate/builtin

import 'dart:async';
import 'dart:io';

import 'package:ed25519_edwards/ed25519_edwards.dart';
import 'package:http/http.dart';
import 'package:logging/logging.dart';
import 'package:path/path.dart' as path;

import 'builder.dart';
import 'crate_hash.dart';
import 'options.dart';
import 'precompile_binaries.dart';
import 'rustup.dart';
import 'target.dart';

class Artifact {
  /// File system location of the artifact.
  final String path;

  /// Actual file name that the artifact should have in destination folder.
  final String finalFileName;

  AritifactType get type {
    if (finalFileName.endsWith('.dll') ||
        finalFileName.endsWith('.dll.lib') ||
        finalFileName.endsWith('.pdb') ||
        finalFileName.endsWith('.so') ||
        finalFileName.endsWith('.dylib')) {
      return AritifactType.dylib;
    } else if (finalFileName.endsWith('.lib') || finalFileName.endsWith('.a')) {
      return AritifactType.staticlib;
    } else {
      throw Exception('Unknown artifact type for $finalFileName');
    }
  }

  Artifact({
    required this.path,
    required this.finalFileName,
  });
}

final _log = Logger('artifacts_provider');

const maxDownloadAttempts = 10;

/// Whether a download error should be retried. Exposed for unit tests.
bool isRetriableDownloadError(Object error) {
  return error is SocketException ||
      error is ClientException ||
      error is HttpException ||
      error is TimeoutException ||
      error is HandshakeException;
}

/// Exponential backoff delay for download retries. Exposed for unit tests.
Duration downloadRetryDelay(int attempt) {
  final seconds = 1 << attempt;
  return Duration(seconds: seconds > 30 ? 30 : seconds);
}

Future<T> retryDownloadRequest<T>(
  Uri url,
  Future<T> Function() action, {
  int maxAttempts = maxDownloadAttempts,
  Duration Function(int attempt)? delayForAttempt,
}) async {
  var attempt = 0;
  while (true) {
    try {
      return await action();
    } catch (e) {
      if (!isRetriableDownloadError(e) || attempt >= maxAttempts - 1) {
        rethrow;
      }
      attempt++;
      _log.warning(
        'Failed to download $url: $e, attempt $attempt of $maxAttempts, will retry...',
      );
      await Future.delayed(
        delayForAttempt?.call(attempt - 1) ?? downloadRetryDelay(attempt - 1),
      );
    }
  }
}

Future<void> downloadArtifactToFile(
  Uri url,
  String destination, {
  Map<String, String>? headers,
}) async {
  await retryDownloadRequest(url, () async {
    final client = Client();
    try {
      final request = Request('GET', url);
      if (headers != null) {
        request.headers.addAll(headers);
      }
      final response = await client.send(request);
      if (response.statusCode != 200) {
        throw ClientException(
          'HTTP ${response.statusCode}',
          url,
        );
      }

      final file = File(destination);
      if (file.existsSync()) {
        file.deleteSync();
      }

      final sink = file.openWrite();
      try {
        await response.stream.pipe(sink);
      } catch (e) {
        await sink.close();
        if (file.existsSync()) {
          file.deleteSync();
        }
        rethrow;
      }
    } finally {
      client.close();
    }
  });
}

class ArtifactProvider {
  ArtifactProvider({
    required this.environment,
    required this.userOptions,
  });

  final BuildEnvironment environment;
  final CargokitUserOptions userOptions;

  Future<Map<Target, List<Artifact>>> getArtifacts(List<Target> targets) async {
    final result = await _getPrecompiledArtifacts(targets);

    final pendingTargets = List.of(targets);
    pendingTargets.removeWhere((element) => result.containsKey(element));

    if (pendingTargets.isEmpty) {
      return result;
    }

    final rustup = Rustup();
    for (final target in pendingTargets) {
      final builder = RustBuilder(target: target, environment: environment);
      builder.prepare(rustup);
      _log.info('Building ${environment.crateInfo.packageName} for $target');
      final targetDir = await builder.build();
      // For local build accept both static and dynamic libraries.
      final artifactNames = <String>{
        ...getArtifactNames(
          target: target,
          libraryName: environment.crateInfo.packageName,
          aritifactType: AritifactType.dylib,
          remote: false,
        ),
        ...getArtifactNames(
          target: target,
          libraryName: environment.crateInfo.packageName,
          aritifactType: AritifactType.staticlib,
          remote: false,
        )
      };
      final artifacts = artifactNames
          .map((artifactName) => Artifact(
                path: path.join(targetDir, artifactName),
                finalFileName: artifactName,
              ))
          .where((element) => File(element.path).existsSync())
          .toList();
      result[target] = artifacts;
    }
    return result;
  }

  Future<Map<Target, List<Artifact>>> _getPrecompiledArtifacts(
      List<Target> targets) async {
    if (userOptions.usePrecompiledBinaries == false) {
      _log.info('Precompiled binaries are disabled');
      return {};
    }
    if (environment.crateOptions.precompiledBinaries == null) {
      _log.fine('Precompiled binaries not enabled for this crate');
      return {};
    }

    final start = Stopwatch()..start();
    final crateHash = CrateHash.compute(environment.manifestDir,
        tempStorage: environment.targetTempDir);
    _log.fine(
        'Computed crate hash $crateHash in ${start.elapsedMilliseconds}ms');

    final downloadedArtifactsDir =
        path.join(environment.targetTempDir, 'precompiled', crateHash);
    Directory(downloadedArtifactsDir).createSync(recursive: true);

    final res = <Target, List<Artifact>>{};

    for (final target in targets) {
      final requiredArtifacts = getArtifactNames(
        target: target,
        libraryName: environment.crateInfo.packageName,
        remote: true,
      );
      final artifactsForTarget = <Artifact>[];

      for (final artifact in requiredArtifacts) {
        final fileName = PrecompileBinaries.fileName(target, artifact);
        final downloadedPath = path.join(downloadedArtifactsDir, fileName);
        if (!File(downloadedPath).existsSync()) {
          final signatureFileName =
              PrecompileBinaries.signatureFileName(target, artifact);
          await _tryDownloadArtifacts(
            crateHash: crateHash,
            fileName: fileName,
            signatureFileName: signatureFileName,
            finalPath: downloadedPath,
          );
        }
        if (File(downloadedPath).existsSync()) {
          artifactsForTarget.add(Artifact(
            path: downloadedPath,
            finalFileName: artifact,
          ));
        } else {
          break;
        }
      }

      // Only provide complete set of artifacts.
      if (artifactsForTarget.length == requiredArtifacts.length) {
        _log.fine('Found precompiled artifacts for $target');
        res[target] = artifactsForTarget;
      }
    }

    return res;
  }

  static Future<Response> _get(Uri url, {Map<String, String>? headers}) {
    return retryDownloadRequest(
      url,
      () => get(url, headers: headers),
    );
  }

  Future<void> _tryDownloadArtifacts({
    required String crateHash,
    required String fileName,
    required String signatureFileName,
    required String finalPath,
  }) async {
    try {
      final precompiledBinaries = environment.crateOptions.precompiledBinaries!;
      final prefix = precompiledBinaries.uriPrefix;
      final url = Uri.parse('$prefix$crateHash/$fileName');
      final signatureUrl = Uri.parse('$prefix$crateHash/$signatureFileName');
      _log.fine('Downloading signature from $signatureUrl');
      final signature = await _get(signatureUrl);
      if (signature.statusCode == 404) {
        _log.warning(
            'Precompiled binaries not available for crate hash $crateHash ($fileName)');
        return;
      }
      if (signature.statusCode != 200) {
        _log.severe(
            'Failed to download signature $signatureUrl: status ${signature.statusCode}');
        return;
      }

      _log.fine('Downloading binary from $url');
      final tempPath = '$finalPath.download';
      final tempFile = File(tempPath);
      try {
        await downloadArtifactToFile(url, tempPath);
        final bodyBytes = tempFile.readAsBytesSync();
        if (verify(precompiledBinaries.publicKey, bodyBytes,
            signature.bodyBytes)) {
          if (File(finalPath).existsSync()) {
            File(finalPath).deleteSync();
          }
          tempFile.renameSync(finalPath);
        } else {
          _log.shout('Signature verification failed! Ignoring binary.');
          tempFile.deleteSync();
        }
      } catch (e) {
        if (tempFile.existsSync()) {
          tempFile.deleteSync();
        }
        rethrow;
      }
    } catch (e, st) {
      _log.warning(
          'Failed to download precompiled artifact $fileName, will fall back to local build: $e');
      _log.fine(st.toString());
    }
  }
}

enum AritifactType {
  staticlib,
  dylib,
}

AritifactType artifactTypeForTarget(Target target) {
  if (target.darwinPlatform != null) {
    return AritifactType.staticlib;
  } else {
    return AritifactType.dylib;
  }
}

List<String> getArtifactNames({
  required Target target,
  required String libraryName,
  required bool remote,
  AritifactType? aritifactType,
}) {
  aritifactType ??= artifactTypeForTarget(target);
  if (target.darwinArch != null) {
    if (aritifactType == AritifactType.staticlib) {
      return ['lib$libraryName.a'];
    } else {
      return ['lib$libraryName.dylib'];
    }
  } else if (target.rust.contains('-windows-')) {
    if (aritifactType == AritifactType.staticlib) {
      return ['$libraryName.lib'];
    } else {
      return [
        '$libraryName.dll',
        '$libraryName.dll.lib',
        if (!remote) '$libraryName.pdb'
      ];
    }
  } else if (target.rust.contains('-linux-')) {
    if (aritifactType == AritifactType.staticlib) {
      return ['lib$libraryName.a'];
    } else {
      return ['lib$libraryName.so'];
    }
  } else {
    throw Exception("Unsupported target: ${target.rust}");
  }
}
