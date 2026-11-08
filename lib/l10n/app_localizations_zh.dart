// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get title => 'KaKa 启动器';

  @override
  String get launch => '启动';

  @override
  String get libraryEmpty => '还没有修改器。\n点击 + 为你的游戏添加修改器 (.exe)。';

  @override
  String get addTrainer => '添加修改器';

  @override
  String get editTrainer => '编辑修改器';

  @override
  String get edit => '编辑';

  @override
  String get gameName => '游戏名称';

  @override
  String get trainerFile => '修改器 (.exe)';

  @override
  String get gameCover => '封面图片（可选）';

  @override
  String get steamAppId => 'Steam AppID（可选）';

  @override
  String get steamAppIdHelp => '仅在同时运行多个游戏时需要：用于选择正确的游戏。';

  @override
  String get select => '选择';

  @override
  String get change => '更改';

  @override
  String get save => '保存';

  @override
  String get cancel => '取消';

  @override
  String get remove => '移除';

  @override
  String removeTrainerConfirm(Object name) {
    return '从库中移除 \"$name\"？\n\n修改器文件本身不会被删除。';
  }

  @override
  String get trainerAdded => '修改器已添加';

  @override
  String get trainerUpdated => '修改器已更新';

  @override
  String get requiredFields => '游戏名称和修改器文件为必填项。';

  @override
  String get invalidAppId => 'Steam AppID 必须是正整数。';

  @override
  String get setting => '设置';

  @override
  String get language => '语言';

  @override
  String get steamPath => 'Steam 路径';

  @override
  String get steamPathAuto => '自动检测';

  @override
  String get set => '设置';

  @override
  String get reset => '重置';

  @override
  String get steamPathSet => 'Steam 路径设置成功';

  @override
  String get ok => '确定';

  @override
  String get launchFail => '启动失败';

  @override
  String launched(Object name) {
    return '修改器已启动：$name';
  }

  @override
  String trainerPathNotFound(Object trainerPath) {
    return '未找到修改器文件。\n\n路径：$trainerPath';
  }

  @override
  String get gameNotRunning => '未找到正在运行的 Proton 游戏。\n\n请先启动游戏（Steam、Heroic、Lutris 或其他 Proton 启动器），再启动修改器。';

  @override
  String multipleGamesRunning(Object games) {
    return '有多个 Proton 游戏正在运行：\n\n$games\n\n请为该修改器设置 Steam AppID 以选择其中一个。';
  }

  @override
  String get gameStarting => '游戏仍在启动中（其 Proton 前缀尚未就绪）。\n\n请几秒后再试。';

  @override
  String protonPathNotFound(Object prefix) {
    return '未找到该游戏的 Proton 安装。\n\n前缀：$prefix';
  }

  @override
  String trainerExited(Object code, Object logPath, Object output) {
    return '修改器立即退出（退出码 $code）。\n\n$output\n\n完整日志：$logPath';
  }

  @override
  String get sourceCode => '源代码';

  @override
  String get basedOn => '基于 LaLa';
}
