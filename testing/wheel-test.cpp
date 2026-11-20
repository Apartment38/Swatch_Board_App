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
  for (int value : {20, 240}) {
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

  // Regression: pure black (#000000) base color must lift V to full brightness
  // on interactive pick so the reticle stays synchronized with the RGB base.
  wheel.setColors({});
  wheel.setColor(QColor(0, 0, 0, 91));
  changes = 0;
  QPointF greenPick(56, 24);
  QMouseEvent blackClick(QEvent::MouseButtonPress, greenPick, greenPick,
                         Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
  QApplication::sendEvent(&wheel, &blackClick);
  if (changes != 1 || last.value() != 255 || last.alpha() != 91 ||
      last.hsvSaturation() < 150 || std::abs(last.hsvHue() - 120) > 3)
    return qCritical("Wheel click on pure black did not lift value to chromatic color"), 1;
  // Round-trip through RGB (as Base color hex field does) must stay chromatic.
  QColor roundTripped(last.red(), last.green(), last.blue(), last.alpha());
  if (roundTripped.value() != 255 || std::abs(roundTripped.hsvHue() - 120) > 3)
    return qCritical("Black-lifted wheel color failed RGB round-trip"), 1;

  // Arrow key on pure black also lifts value while preserving alpha.
  wheel.setColor(QColor(0, 0, 0, 255));
  changes = 0;
  QKeyEvent upKey(QEvent::KeyPress, Qt::Key_Up, Qt::NoModifier);
  QApplication::sendEvent(&wheel, &upKey);
  if (changes != 1 || last.value() != 255 || last.alpha() != 255 ||
      last.hsvSaturation() <= 0)
    return qCritical("Wheel arrow key on pure black did not lift value"), 1;

  // Regression: harmony spokes and dashed base spoke must be visible on the
  // bright wheel center under a dark UI theme (Breeze Dark palette).
  QPalette darkPal = wheel.palette();
  darkPal.setColor(QPalette::Window, QColor(0x20, 0x23, 0x26));
  darkPal.setColor(QPalette::WindowText, QColor(0xEF, 0xF0, 0xF1));
  darkPal.setColor(QPalette::Text, QColor(0xEF, 0xF0, 0xF1));
  wheel.setPalette(darkPal);
  wheel.setColor(QColor::fromHsv(0, 255, 255));       // base spoke right (+x)
  wheel.setColors({QColor::fromHsv(180, 255, 255)});  // harmony spoke left (-x)
  QApplication::processEvents();
  QImage darkThemeWheel = wheel.grab().toImage();
  if (qEnvironmentVariableIsSet("SWATCH_WHEEL_SCREENSHOT_DIR"))
    darkThemeWheel.save(qEnvironmentVariable("SWATCH_WHEEL_SCREENSHOT_DIR") + "/dark-theme.png");
  // Sample along the harmony spoke (x=80..92, y=100) and dashed base spoke
  // (x=108..126, y=100) near the white center; both must have dark ink pixels.
  int minHarmonyLuma = 255;
  for (int x = 80; x <= 92; ++x)
    minHarmonyLuma = qMin(minHarmonyLuma, qGray(darkThemeWheel.pixel(x, 100)));
  int minBaseLuma = 255;
  for (int x = 108; x <= 126; ++x)
    minBaseLuma = qMin(minBaseLuma, qGray(darkThemeWheel.pixel(x, 100)));
  if (minHarmonyLuma > 180)
    return qCritical("Harmony spoke invisible under dark theme (min luma=%d)", minHarmonyLuma), 1;
  if (minBaseLuma > 180)
    return qCritical("Dashed base spoke invisible under dark theme (min luma=%d)", minBaseLuma), 1;
  return 0;
}
