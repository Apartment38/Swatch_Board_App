#include "../rust/qt/src/generator_wheel.h"
#include <QtWidgets>
#include <cmath>

int main(int argc, char **argv) {
  QApplication app(argc, argv);
  QColor last;
  int changes = 0;
  GeneratorWheel wheel(QColor(Qt::red), [&](QColor c) {
    last = c;
    ++changes;
  }, nullptr);
  wheel.resize(200, 200);
  wheel.show();
  QApplication::processEvents();

  QKeyEvent right(QEvent::KeyPress, Qt::Key_Right, Qt::NoModifier);
  QApplication::sendEvent(&wheel, &right);
  if (changes != 1 || last.hsvHue() != 1)
    return qCritical("Wheel right arrow did not adjust base hue"), 1;

  wheel.setColor(QColor(Qt::red));
  wheel.setColors({QColor(Qt::green)});
  wheel.setHarmonyHandles(true);
  changes = 0;
  // The green marker lies 120 degrees around the 88-pixel wheel radius.
  QPointF green(56, 24), up(100, 12);
  QMouseEvent press(QEvent::MouseButtonPress, green, green, Qt::LeftButton,
                    Qt::LeftButton, Qt::NoModifier);
  QApplication::sendEvent(&wheel, &press);
  if (changes != 0)
    return qCritical("Pressing a harmony marker unexpectedly changed the base"), 1;
  QMouseEvent drag(QEvent::MouseMove, up, up, Qt::NoButton, Qt::LeftButton,
                   Qt::NoModifier);
  QApplication::sendEvent(&wheel, &drag);
  // Dragging from green (120°) to the top (90°) rotates the red base by -30°.
  if (changes != 1 || std::abs(last.hsvHue() - 330) > 2)
    return qCritical("Dragging a harmony marker did not rotate the base"), 1;
  return 0;
}
