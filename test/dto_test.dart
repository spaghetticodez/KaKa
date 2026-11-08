import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/util/dto.dart';

void main() {
  test('CustomGame survives a JSON round trip', () {
    final game = CustomGame(name: 'Test Game', appId: 1234567890, coverImagePath: '/tmp/cover.png', trainerPath: '/tmp/trainer.exe');
    final restored = CustomGame.fromJson(game.toJson());
    expect(restored.name, game.name);
    expect(restored.appId, game.appId);
    expect(restored.coverImagePath, game.coverImagePath);
    expect(restored.trainerPath, game.trainerPath);
  });
}
