#pragma once

#include <QtWidgets>
#include <cmath>
#include <functional>

// An interactive HSV base-color picker for the palette generator.
class GeneratorWheel : public QWidget {
  QColor selected;
  std::function<void(QColor)> changed;

  QRectF wheelRect() const {
    const int side = qMax(1, qMin(width(), height()) - 16);
    return QRectF((width() - side) / 2.0, (height() - side) / 2.0, side, side);
  }
  void choose(const QPointF &point) {
    const QRectF r = wheelRect();
    const QPointF d = point - r.center();
    const double radius = r.width() / 2.0;
    const double saturation = qMin(1.0, std::hypot(d.x(), d.y()) / radius);
    double angle = std::atan2(-d.y(), d.x()) * 180.0 / M_PI;
    if (angle < 0) angle += 360;
    selected.setHsvF(angle / 360.0, saturation, selected.value() == 0 ? 1.0 : selected.valueF(), selected.alphaF());
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
    setAccessibleName("Palette base color wheel; arrow keys adjust hue and saturation");
  }
  void setColor(QColor value) { if (value.isValid()) { selected = value; update(); } }
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
    double hue = selected.hsvHueF();
    if (hue < 0) hue = 0;
    double angle = hue * 2 * M_PI;
    QPointF marker = r.center() + QPointF(std::cos(angle), -std::sin(angle)) * (selected.hsvSaturationF() * r.width() / 2);
    p.setPen(QPen(Qt::black, 3)); p.drawEllipse(marker, 6, 6);
    p.setPen(QPen(Qt::white, 1.5)); p.drawEllipse(marker, 6, 6);
  }
  void mousePressEvent(QMouseEvent *e) override { if (e->button() == Qt::LeftButton) choose(e->position()); }
  void mouseMoveEvent(QMouseEvent *e) override { if (e->buttons() & Qt::LeftButton) choose(e->position()); }
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
