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
  // The HS map stays bright for dark colors, while interaction preserves value
  // and alpha. Sample away from markers and spokes.
  wheel.setColors({});
  wheel.setColor(QColor::fromHsv(30, 200, 20, 91));
  QApplication::processEvents();
  auto darkImage = wheel.grab();
  if (qEnvironmentVariableIsSet("SWATCH_WHEEL_SCREENSHOT_DIR"))
    darkImage.save(qEnvironmentVariable("SWATCH_WHEEL_SCREENSHOT_DIR") + "/dark.png");
  QColor darkMap = darkImage.toImage().pixelColor(140, 70);
  wheel.setColor(QColor::fromHsv(30, 200, 240, 91));
  QApplication::processEvents();
  auto brightImage = wheel.grab();
  if (qEnvironmentVariableIsSet("SWATCH_WHEEL_SCREENSHOT_DIR"))
    brightImage.save(qEnvironmentVariable("SWATCH_WHEEL_SCREENSHOT_DIR") + "/bright.png");
  QColor brightMap = brightImage.toImage().pixelColor(140, 70);
  if (darkMap != brightMap || darkMap.value() < 240 || darkMap.hsvSaturation() < 80)
    return qCritical("Hue/saturation map depends on base brightness"), 1;
  for (int value : {0, 20, 240}) {
    wheel.setColor(QColor::fromHsv(30, 200, value, 91));
    changes = 0;
    QApplication::sendEvent(&wheel, &right);
    if (changes != 1 || last.value() != value || last.alpha() != 91)
      return qCritical("Wheel key changed base value or alpha"), 1;
    QPointF point(160, 100);
    QMouseEvent click(QEvent::MouseButtonPress, point, point, Qt::LeftButton,
                      Qt::LeftButton, Qt::NoModifier);
    QApplication::sendEvent(&wheel, &click);
    if (changes != 2 || last.value() != value || last.alpha() != 91)
      return qCritical("Wheel click changed base value or alpha"), 1;
  }
  return 0;
}
