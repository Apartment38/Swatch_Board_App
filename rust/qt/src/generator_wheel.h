#pragma once

#include <QtWidgets>
#include <cmath>
#include <functional>

// The markers show the actual generated colors in HSV coordinates. Dragging a
// marker rotates the base color; the generator recomputes the harmony.
class GeneratorWheel : public QWidget {
  QColor selected;
  QList<QColor> colors;
  std::function<void(QColor)> changed;
  QPointF lastDrag;
  bool draggingHandle = false;
  bool harmonyHandles = true;

  QRectF wheelRect() const {
    const int side = qMax(1, qMin(width(), height()) - 24);
    return QRectF((width() - side) / 2.0, (height() - side) / 2.0, side, side);
  }
  QPointF marker(const QColor &c) const {
    const auto r = wheelRect();
    double h = c.hsvHueF();
    if (h < 0) h = 0;
    double a = h * 2 * M_PI;
    return r.center() + QPointF(std::cos(a), -std::sin(a)) * (c.hsvSaturationF() * r.width() / 2);
  }
  void choose(const QPointF &point, bool relative) {
    const auto r = wheelRect();
    auto polar = [r](QPointF p) {
      QPointF d = p - r.center();
      double angle = std::atan2(-d.y(), d.x()) / (2 * M_PI);
      if (angle < 0) angle += 1;
      return QPointF(angle, qMin(1.0, std::hypot(d.x(), d.y()) / (r.width() / 2)));
    };
    const auto next = polar(point);
    double h = next.x(), s = next.y();
    if (relative) {
      const auto old = polar(lastDrag);
      double originalHue = selected.hsvHueF();
      if (originalHue < 0) originalHue = 0;
      h = std::fmod(originalHue + next.x() - old.x() + 1.0, 1.0);
      s = qBound(0.0, selected.hsvSaturationF() + next.y() - old.y(), 1.0);
    }
    lastDrag = point;
    selected.setHsvF(h, s, selected.valueF(), selected.alphaF());
    update();
    changed(selected);
  }
public:
  GeneratorWheel(QColor initial, std::function<void(QColor)> callback, QWidget *parent)
      : QWidget(parent), selected(initial.isValid() ? initial : QColor(Qt::red)), changed(callback) {
    setMinimumSize(160, 160);
    setMaximumSize(280, 280);
    setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Preferred);
    setFocusPolicy(Qt::StrongFocus);
    setCursor(Qt::CrossCursor);
    setAccessibleName("Palette hue and saturation wheel; arrow keys adjust base hue and saturation");
    setToolTip("Hue and saturation map at full brightness. Drag a generated color to rotate the harmony; arrow keys adjust the base color.");
  }
  void setColor(QColor value) { if (value.isValid()) { selected = value; update(); } }
  void setColors(const QList<QColor> &values) { colors = values; update(); }
  void setHarmonyHandles(bool enabled) {
    harmonyHandles = enabled;
    setToolTip(enabled ? "Hue and saturation map at full brightness. Drag a generated color to rotate the harmony; arrow keys adjust the base color."
                       : "Hue and saturation map at full brightness. Choose a base hue and saturation with the wheel or arrow keys. This mode has no fixed harmony handles.");
  }
protected:
  void paintEvent(QPaintEvent *) override {
    const QRectF r = wheelRect();
    const qreal dpr = devicePixelRatioF();
    const int side = qMax(1, qRound(r.width() * dpr));
    QImage image(side, side, QImage::Format_ARGB32_Premultiplied);
    image.setDevicePixelRatio(dpr);
    image.fill(Qt::transparent);
    const double radius = side / 2.0;
    const double aaBand = qMax(1.0, dpr);
    for (int y = 0; y < side; ++y) {
      for (int x = 0; x < side; ++x) {
        const double dx = x + 0.5 - radius;
        const double dy = y + 0.5 - radius;
        const double dist = std::hypot(dx, dy);
        if (dist <= radius + 0.5) {
          const double sat = qBound(0.0, dist / radius, 1.0);
          double hue = std::atan2(-dy, dx) / (2.0 * M_PI);
          if (hue < 0.0) hue += 1.0;
          // This is a hue/saturation map; value and alpha remain in the base color.
          const double edgeAlpha = qBound(0.0, (radius + 0.5 - dist) / aaBand, 1.0);
          QColor px = QColor::fromHsvF(hue, sat, 1.0);
          px.setAlphaF(edgeAlpha);
          image.setPixelColor(x, y, px);
        }
      }
    }
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.setRenderHint(QPainter::SmoothPixmapTransform, true);
    p.drawImage(r.topLeft(), image);
    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    p.setPen(QPen(darkSurface ? QColor(255, 255, 255, 58) : QColor(0, 0, 0, 42), 1.0));
    p.setBrush(Qt::NoBrush);
    p.drawEllipse(r.adjusted(0.5, 0.5, -0.5, -0.5));
    QColor spoke = palette().color(QPalette::Text);
    spoke.setAlpha(135);
    p.setPen(QPen(spoke, 1.1));
    for (const auto &c : colors) p.drawLine(r.center(), marker(c));
    for (int i = 0; i < colors.size(); ++i) {
      const auto &c = colors[i];
      const auto point = marker(c);
      p.setBrush(c);
      p.setPen(QPen(QColor(0, 0, 0, 210), 1.8));
      p.drawEllipse(point, 7.5, 7.5);
      p.setBrush(Qt::NoBrush);
      p.setPen(QPen(Qt::white, 1.4));
      p.drawEllipse(point, 6.3, 6.3);
    }
    const QPointF basePoint = marker(selected);
    p.setBrush(Qt::NoBrush);
    p.setPen(QPen(QColor(0, 0, 0, 225), 1.5));
    p.drawEllipse(basePoint, 5.8, 5.8);
    p.setPen(QPen(Qt::white, 1.6));
    p.drawEllipse(basePoint, 4.6, 4.6);
    p.setPen(QPen(QColor(0, 0, 0, 190), 1.0));
    p.drawEllipse(basePoint, 3.4, 3.4);
    if (hasFocus()) {
      p.setPen(QPen(palette().color(QPalette::Highlight), 2.0));
      p.setBrush(Qt::NoBrush);
      p.drawRoundedRect(QRectF(rect()).adjusted(1.0, 1.0, -1.0, -1.0), 3.0, 3.0);
    }
  }
  void mousePressEvent(QMouseEvent *e) override {
    if (e->button() != Qt::LeftButton) return;
    setFocus(Qt::MouseFocusReason);
    draggingHandle = false;
    if (harmonyHandles)
      for (const auto &c : colors)
        if (QLineF(marker(c), e->position()).length() <= 12) { draggingHandle = true; break; }
    lastDrag = e->position();
    if (!draggingHandle) choose(e->position(), false);
  }
  void mouseMoveEvent(QMouseEvent *e) override { if (e->buttons() & Qt::LeftButton) choose(e->position(), draggingHandle); }
  void keyPressEvent(QKeyEvent *e) override {
    int h = selected.hsvHue(); if (h < 0) h = 0;
    int s = selected.hsvSaturation();
    if (e->key() == Qt::Key_Left) h = (h + 359) % 360;
    else if (e->key() == Qt::Key_Right) h = (h + 1) % 360;
    else if (e->key() == Qt::Key_Up) s = qMin(255, s + 3);
    else if (e->key() == Qt::Key_Down) s = qMax(0, s - 3);
    else { QWidget::keyPressEvent(e); return; }
    selected.setHsv(h, s, selected.value(), selected.alpha()); update(); changed(selected);
  }
};
