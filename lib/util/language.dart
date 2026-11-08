const List<String> languageOptions = ['English', '中文', 'German'];

String languageToLocale(String language) {
  switch (language) {
    case '中文':
      return 'zh';
    case 'German':
      return 'de';
    default:
      return 'en';
  }
}
