#include "my_application.h"

int main(int argc, char** argv) {
  // GTK3 derives the Wayland app_id and the X11 WM_CLASS instance name from the
  // program name. Use the application id, so the window matches
  // io.github.spaghetticodez.kaka.desktop (icon, taskbar grouping).
  g_set_prgname(APPLICATION_ID);

  g_autoptr(MyApplication) app = my_application_new();
  return g_application_run(G_APPLICATION(app), argc, argv);
}
