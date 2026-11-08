import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:kaka/util/trainer_data.dart';
import 'package:path/path.dart' as p;

void main() {
  late Directory tmp;
  late String prefix;
  late String local;
  late Directory outside;

  File touch(String path) => File(path)
    ..createSync(recursive: true)
    ..writeAsStringSync('x');

  setUp(() {
    tmp = Directory.systemTemp.createTempSync('kaka_cleanup_');
    prefix = p.join(tmp.path, 'compatdata', '1245620');
    local = p.join(prefix, 'pfx', 'drive_c', 'users', 'steamuser', 'AppData', 'Local');
    outside = Directory(p.join(tmp.path, 'outside'))..createSync();
    touch(p.join(outside.path, 'precious.txt'));
    touch(p.join(prefix, 'pfx.lock'));
    touch(p.join(prefix, 'version'));
  });

  tearDown(() => tmp.deleteSync(recursive: true));

  bool exists(String path) => FileSystemEntity.typeSync(path, followLinks: false) != FileSystemEntityType.notFound;

  test('removes FLiNGTrainer, empties Temp, and touches nothing else', () {
    touch(p.join(local, 'FLiNGTrainer', 'state.dat'));
    touch(p.join(local, 'FLiNGTrainer', 'sub', 'x'));
    touch(p.join(local, 'Temp', 'a.tmp'));
    touch(p.join(local, 'Temp', 'dir', 'b'));
    Link(p.join(local, 'Temp', 'escape')).createSync(outside.path);
    Link(p.join(local, 'Temp', 'dir', 'nested-escape')).createSync(outside.path);
    touch(p.join(local, 'GameSaves', 'save.sav'));

    expect(TrainerDataCleanup(prefix).run(inUse: false), 4);

    expect(exists(p.join(local, 'FLiNGTrainer')), isFalse);
    expect(Directory(p.join(local, 'Temp')).existsSync(), isTrue, reason: 'Temp itself stays');
    expect(Directory(p.join(local, 'Temp')).listSync(), isEmpty);
    expect(File(p.join(local, 'GameSaves', 'save.sav')).existsSync(), isTrue);
    expect(File(p.join(prefix, 'pfx.lock')).existsSync(), isTrue, reason: 'pfx.lock is never touched');
    expect(File(p.join(prefix, 'version')).existsSync(), isTrue);
    expect(File(p.join(outside.path, 'precious.txt')).existsSync(), isTrue, reason: 'symlink targets are never followed');
  });

  test('refuses while the prefix is in use and deletes nothing', () {
    touch(p.join(local, 'FLiNGTrainer', 'state.dat'));
    touch(p.join(local, 'Temp', 'a.tmp'));
    expect(() => TrainerDataCleanup(prefix).run(inUse: true),
        throwsA(isA<CleanupRefused>().having((e) => e.reason, 'reason', CleanupRefusal.prefixInUse)));
    expect(File(p.join(local, 'FLiNGTrainer', 'state.dat')).existsSync(), isTrue);
    expect(File(p.join(local, 'Temp', 'a.tmp')).existsSync(), isTrue);
  });

  test('refuses when AppData/Local resolves outside the prefix', () {
    touch(p.join(outside.path, 'FLiNGTrainer', 'state.dat'));
    touch(p.join(outside.path, 'Temp', 'a.tmp'));
    Directory(p.dirname(local)).createSync(recursive: true);
    Link(local).createSync(outside.path);

    expect(() => TrainerDataCleanup(prefix).run(inUse: false),
        throwsA(isA<CleanupRefused>().having((e) => e.reason, 'reason', CleanupRefusal.outsidePrefix)));
    expect(File(p.join(outside.path, 'FLiNGTrainer', 'state.dat')).existsSync(), isTrue);
    expect(File(p.join(outside.path, 'Temp', 'a.tmp')).existsSync(), isTrue);
  });

  test('a FLiNGTrainer symlink is removed as a link, its target survives', () {
    Directory(local).createSync(recursive: true);
    Link(p.join(local, 'FLiNGTrainer')).createSync(outside.path);
    expect(TrainerDataCleanup(prefix).run(inUse: false), 1);
    expect(exists(p.join(local, 'FLiNGTrainer')), isFalse);
    expect(File(p.join(outside.path, 'precious.txt')).existsSync(), isTrue);
  });

  test('a prefix without the folders is a no-op', () {
    expect(TrainerDataCleanup(prefix).run(inUse: false), 0);
    expect(TrainerDataCleanup(p.join(tmp.path, 'missing')).run(inUse: false), 0);
  });

  test('targets are the documented FLiNG locations of the Proton user', () {
    expect(TrainerDataCleanup('/c/1').targets, [
      '/c/1/pfx/drive_c/users/steamuser/AppData/Local/FLiNGTrainer',
      '/c/1/pfx/drive_c/users/steamuser/AppData/Local/Temp',
    ]);
  });
}
