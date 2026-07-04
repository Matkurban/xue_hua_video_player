import 'package:build_tool/src/options.dart';
import 'package:build_tool/src/rustup.dart';
import 'package:test/test.dart';
import 'package:yaml/yaml.dart';

void main() {
  test('defaults to precompiled binaries even when rustup is installed', () {
    rustupExecutablePathOverride = () => '/tmp/fake-rustup';
    addTearDown(() => rustupExecutablePathOverride = null);

    expect(CargokitUserOptions.defaultUsePrecompiledBinaries(), isTrue);
  });

  test('explicit opt-out still disables precompiled binaries', () {
    final options = CargokitUserOptions.parse(
      loadYamlNode('use_precompiled_binaries: false'),
    );

    expect(options.usePrecompiledBinaries, isFalse);
  });
}
