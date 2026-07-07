import 'package:build_tool/src/options.dart';
import 'package:test/test.dart';
import 'package:yaml/yaml.dart';

void main() {
  test('parses verbose_logging from cargokit_options.yaml', () {
    final options = CargokitUserOptions.parse(
      loadYamlNode('verbose_logging: true'),
    );

    expect(options.verboseLogging, isTrue);
  });
}
