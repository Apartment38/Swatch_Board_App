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
    selected.setHsvF(h, s, selected.value() == 0 ? 1.0 : selected.valueF(), selected.alphaF());
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
    setAccessibleName("Palette harmony wheel; arrow keys adjust base hue and saturation");
    setToolTip("Drag a generated color to rotate the harmony; arrow keys adjust the base color.");
  }
  void setColor(QColor value) { if (value.isValid()) { selected = value; update(); } }
  void setColors(const QList<QColor> &values) { colors = values; update(); }
  void setHarmonyHandles(bool enabled) {
    harmonyHandles = enabled;
    setToolTip(enabled ? "Drag a generated color to rotate the harmony; arrow keys adjust the base color."
                       : "Choose a base color with the wheel or arrow keys. This mode has no fixed harmony handles.");
  }
protected:
  void paintEvent(QPaintEvent *) override {
    const QRectF r = wheelRect();
    const int side = qMax(1, int(r.width()));
    QImage image(side, side, QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::transparent);
    const double radius = side / 2.0;
    for (int y = 0; y < side; ++y) for (int x = 0; x < side; ++x) {
      double dx = x + .5 - radius, dy = y + .5 - radius;
      double sat = std::hypot(dx, dy) / radius;
      if (sat <= 1.0) {
        double hue = std::atan2(-dy, dx) / (2 * M_PI);
        if (hue < 0) hue += 1;
        image.setPixelColor(x, y, QColor::fromHsvF(hue, sat, selected.value() == 0 ? 1.0 : selected.valueF()));
      }
    }
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);
    p.drawImage(r.topLeft(), image);
    p.setPen(QPen(palette().color(QPalette::Text), 1));
    for (const auto &c : colors) p.drawLine(r.center(), marker(c));
    for (int i = 0; i < colors.size(); ++i) {
      const auto &c = colors[i];
      auto point = marker(c);
      p.setBrush(c); p.setPen(QPen(Qt::black, 2)); p.drawEllipse(point, 8, 8);
      p.setPen(QPen(Qt::white, 1)); p.drawEllipse(point, 7, 7);
    }
    p.setBrush(Qt::NoBrush);
    p.setPen(QPen(Qt::black, 3)); p.drawEllipse(marker(selected), 5, 5);
    p.setPen(QPen(Qt::white, 1.5)); p.drawEllipse(marker(selected), 5, 5);
    if (hasFocus()) { p.setPen(QPen(palette().color(QPalette::Highlight), 2)); p.drawRect(rect().adjusted(1, 1, -2, -2)); }
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
    selected.setHsv(h, s, selected.value() == 0 ? 255 : selected.value(), selected.alpha()); update(); changed(selected);
  }
};
