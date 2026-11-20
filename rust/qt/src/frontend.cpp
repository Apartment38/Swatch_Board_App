#include <QtCore>
#include <QtGui>
#include <QtWidgets>
#include <functional>
#include <limits>
#include "generator_wheel.h"

extern "C" void swatch_qt_free(char *);
using Callback = char *(*)(void *, const char *);
static QColor color(const QString &s) {
  if (s.size() == 9 && s[0] == '#') {
    bool ok;
    uint v = s.mid(1).toUInt(&ok, 16);
    if (ok)
      return QColor((v >> 24) & 255, (v >> 16) & 255, (v >> 8) & 255, v & 255);
  }
  return QColor(s);
}
static QString hex(const QColor &c) {
  return QString("#%1%2%3%4")
      .arg(c.red(), 2, 16, QChar('0'))
      .arg(c.green(), 2, 16, QChar('0'))
      .arg(c.blue(), 2, 16, QChar('0'))
      .arg(c.alpha(), 2, 16, QChar('0'))
      .toUpper();
}
static QString str(const QJsonObject &o, const char *key) {
  return o.value(QLatin1String(key)).toString();
}
static QJsonObject object(const QJsonValue &v) { return v.toObject(); }
static QJsonArray array(const QJsonObject &o, const char *key) {
  return o.value(QLatin1String(key)).toArray();
}
static QPushButton *button(const QString &label, QWidget *parent,
                           const std::function<void()> &f) {
  auto b = new QPushButton(label, parent);
  b->setAccessibleName(label);
  QObject::connect(b, &QPushButton::clicked, parent, f);
  return b;
}
static QLineEdit *line(const QString &name, const QString &value,
                       QWidget *parent) {
  auto e = new QLineEdit(value, parent);
  e->setAccessibleName(name);
  return e;
}
static QSpinBox *spin(const QString &name, int value, int min, int max,
                      QWidget *parent) {
  auto s = new QSpinBox(parent);
  s->setAccessibleName(name);
  s->setRange(min, max);
  s->setValue(value);
  return s;
}
static QDoubleSpinBox *decimal(const QString &name, double value, double min,
                               double max, QWidget *parent) {
  auto s = new QDoubleSpinBox(parent);
  s->setAccessibleName(name);
  s->setDecimals(3);
  s->setRange(min, max);
  s->setSingleStep(.01);
  s->setValue(value);
  return s;
}

static void paintCheckerboard(QPainter &p, const QRect &r, int cell = 6) {
  static const QColor light(0xE4, 0xE4, 0xE4);
  static const QColor dark(0xCC, 0xCC, 0xCC);
  p.fillRect(r, light);
  for (int y = r.top(); y <= r.bottom(); y += cell) {
    const int row = (y - r.top()) / cell;
    for (int x = r.left(); x <= r.right(); x += cell) {
      const int col = (x - r.left()) / cell;
      if ((row + col) & 1) {
        p.fillRect(QRect(x, y, qMin(cell, r.right() - x + 1),
                         qMin(cell, r.bottom() - y + 1)),
                   dark);
      }
    }
  }
}

static QColor mutedColor(const QPalette &pal, int alpha = 145) {
  const QColor fg = pal.color(QPalette::WindowText);
  const QColor bg = pal.color(QPalette::Window);
  const qreal t = qBound(0.0, alpha / 255.0, 1.0);
  return QColor::fromRgbF(fg.redF() * t + bg.redF() * (1.0 - t),
                          fg.greenF() * t + bg.greenF() * (1.0 - t),
                          fg.blueF() * t + bg.blueF() * (1.0 - t));
}

class Hairline : public QFrame {
public:
  explicit Hairline(QWidget *parent = nullptr)
      : Hairline(Qt::Horizontal, parent) {}
  Hairline(Qt::Orientation orientation, QWidget *parent = nullptr)
      : QFrame(parent) {
    if (orientation == Qt::Vertical) {
      setFrameShape(QFrame::VLine);
      setFixedWidth(1);
      setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
    } else {
      setFrameShape(QFrame::HLine);
      setFixedHeight(1);
      setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    }
  }

protected:
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    QColor c = palette().color(QPalette::Mid);
    c.setAlpha(155);
    p.fillRect(rect(), c);
  }
};

class StatusDot : public QWidget {
  QColor dotColor;

public:
  static QColor neutralColor(const QPalette &pal) {
    return mutedColor(pal, 155);
  }
  explicit StatusDot(QWidget *parent = nullptr) : QWidget(parent) {
    setFixedSize(8, 8);
    dotColor = neutralColor(palette());
  }
  void setColor(const QColor &value) {
    if (dotColor != value) {
      dotColor = value;
      update();
    }
  }

protected:
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    QColor border = darkSurface ? QColor(255, 255, 255, 65) : dotColor.darker(125);
    if (!darkSurface)
      border.setAlpha(180);
    p.setPen(QPen(border, 1.0));
    p.setBrush(dotColor);
    p.drawEllipse(QRectF(1.0, 1.0, 6.0, 6.0));
  }
};

class InlineBanner : public QWidget {
  QColor tintColor;
  int fillAlpha;
  int borderAlpha;

public:
  InlineBanner(const QColor &tint, int fillA = 24, int borderA = 145,
               QWidget *parent = nullptr)
      : QWidget(parent), tintColor(tint), fillAlpha(fillA),
        borderAlpha(borderA) {}

protected:
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    QColor fill = tintColor;
    fill.setAlpha(fillAlpha);
    QColor border = tintColor;
    border.setAlpha(borderAlpha);
    p.setBrush(fill);
    p.setPen(QPen(border, 1.0));
    p.drawRoundedRect(QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5), 4.0, 4.0);
  }
};

class HistoryLimitSpinBox : public QAbstractSpinBox {
  static constexpr quint64 kMaxHistoryLimit = 9223372036854775807ULL;
  bool invalid = false;

  enum class ParsedState { EmptyOrZero, Valid, Overflow };
  ParsedState parseValue(quint64 &out) const {
    const QString t = lineEdit()->text().trimmed();
    if (t.isEmpty()) {
      out = 0;
      return ParsedState::EmptyOrZero;
    }
    bool ok = false;
    const quint64 v = t.toULongLong(&ok);
    if (!ok || v > kMaxHistoryLimit) {
      out = kMaxHistoryLimit;
      return ParsedState::Overflow;
    }
    out = v;
    return v == 0 ? ParsedState::EmptyOrZero : ParsedState::Valid;
  }

public:
  explicit HistoryLimitSpinBox(const QString &initialValue,
                               QWidget *parent = nullptr)
      : QAbstractSpinBox(parent) {
    setAccessibleName("Stored history limit");
    setButtonSymbols(QAbstractSpinBox::UpDownArrows);
    setKeyboardTracking(true);
    lineEdit()->setAccessibleName("Stored history limit");
    lineEdit()->setAutoFillBackground(false);
    lineEdit()->setAttribute(Qt::WA_OpaquePaintEvent, false);
    QPalette editPal = lineEdit()->palette();
    editPal.setColor(QPalette::Base, Qt::transparent);
    lineEdit()->setPalette(editPal);
    lineEdit()->setValidator(new QRegularExpressionValidator(
        QRegularExpression("[0-9]{1,20}"), this));
    lineEdit()->setText(initialValue);
    connect(lineEdit(), &QLineEdit::textChanged, this,
            [this](const QString &) { update(); });
  }
  using QAbstractSpinBox::lineEdit;
  void setInvalid(bool bad) {
    if (invalid != bad) {
      invalid = bad;
      update();
    }
  }
  void stepBy(int steps) override {
    if (isReadOnly() || steps == 0)
      return;
    quint64 v = 0;
    const auto st = parseValue(v);
    quint64 next = v;
    if (st == ParsedState::Overflow) {
      if (steps > 0)
        return;
      const quint64 extra =
          static_cast<quint64>(-static_cast<qint64>(steps) - 1);
      next = (extra < kMaxHistoryLimit) ? (kMaxHistoryLimit - extra) : 1;
      if (next < 1)
        next = 1;
    } else if (steps > 0) {
      const quint64 delta = static_cast<quint64>(steps);
      next = (v >= kMaxHistoryLimit - delta) ? kMaxHistoryLimit : (v + delta);
      if (next < 1)
        next = 1;
    } else {
      const quint64 delta = static_cast<quint64>(-static_cast<qint64>(steps));
      next = (v > delta && v - delta >= 1) ? (v - delta) : 1;
    }
    lineEdit()->setText(QString::number(next));
    update();
  }

protected:
  StepEnabled stepEnabled() const override {
    if (isReadOnly())
      return StepNone;
    quint64 v = 0;
    const auto st = parseValue(v);
    if (st == ParsedState::Overflow)
      return StepDownEnabled;
    StepEnabled flags = StepNone;
    if (v < kMaxHistoryLimit)
      flags |= StepUpEnabled;
    if (v > 1)
      flags |= StepDownEnabled;
    return flags;
  }
  void paintEvent(QPaintEvent *e) override {
    QAbstractSpinBox::paintEvent(e);
    if (!invalid)
      return;
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.setBrush(QColor(218, 68, 83, 14));
    p.setPen(QPen(QColor(218, 68, 83, 215), 1.5));
    p.drawRoundedRect(QRectF(rect()).adjusted(0.75, 0.75, -0.75, -0.75), 4.0,
                      4.0);
  }
};

class NavToolButton : public QToolButton {
  bool pointerHovered = false;

public:
  explicit NavToolButton(QWidget *parent = nullptr) : QToolButton(parent) {
    setAutoRaise(true);
    setFocusPolicy(Qt::NoFocus);
    setMouseTracking(true);
    setMinimumWidth(0);
    setSizePolicy(QSizePolicy::Minimum, QSizePolicy::Preferred);
  }
  QSize sizeHint() const override {
    ensurePolished();
    const bool showText =
        toolButtonStyle() != Qt::ToolButtonIconOnly && !text().isEmpty();
    const QSize iconSz = icon().isNull() ? QSize(0, 0) : iconSize();
    const int textW = showText ? fontMetrics().horizontalAdvance(text()) : 0;
    const int textH = showText ? fontMetrics().height() : 0;
    const int gap = (!icon().isNull() && showText) ? 5 : 0;
    const int padX = showText ? 8 : 6;
    const int padY = 4;
    const int w = iconSz.width() + gap + textW + 2 * padX;
    const int h = qMax(iconSz.height(), textH) + 2 * padY;
    return QSize(w, h);
  }
  QSize minimumSizeHint() const override { return sizeHint(); }

protected:
  void mouseMoveEvent(QMouseEvent *e) override {
    if (!pointerHovered) {
      pointerHovered = true;
      update();
    }
    QToolButton::mouseMoveEvent(e);
  }
  void leaveEvent(QEvent *e) override {
    if (pointerHovered) {
      pointerHovered = false;
      update();
    }
    QToolButton::leaveEvent(e);
  }
  void hideEvent(QHideEvent *e) override {
    if (pointerHovered) {
      pointerHovered = false;
      update();
    }
    QToolButton::hideEvent(e);
  }
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);

    const bool checked = isChecked();
    const bool pressed = isDown();
    const bool hovered = pointerHovered && isEnabled();
    const QColor hi = palette().color(QPalette::Active, QPalette::Highlight);
    const QColor fg = palette().color(
        isEnabled() ? QPalette::Active : QPalette::Disabled,
        QPalette::WindowText);

    const QRectF pill = QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5);
    if (checked) {
      QColor fill = hi;
      fill.setAlpha(pressed ? 62 : (hovered ? 56 : 42));
      QColor border = hi;
      border.setAlpha(pressed ? 215 : (hovered ? 205 : 185));
      p.setBrush(fill);
      p.setPen(QPen(border, 1.0));
      p.drawRoundedRect(pill, 3.5, 3.5);
    } else if (pressed) {
      QColor fill = fg;
      fill.setAlpha(24);
      p.setBrush(fill);
      p.setPen(Qt::NoPen);
      p.drawRoundedRect(pill, 3.5, 3.5);
    } else if (hovered) {
      QColor fill = fg;
      fill.setAlpha(14);
      p.setBrush(fill);
      p.setPen(Qt::NoPen);
      p.drawRoundedRect(pill, 3.5, 3.5);
    }

    const bool showText =
        toolButtonStyle() != Qt::ToolButtonIconOnly && !text().isEmpty();
    const QSize iconSz = icon().isNull() ? QSize(0, 0) : iconSize();
    const QFontMetrics fm = fontMetrics();
    const int textW = showText ? fm.horizontalAdvance(text()) : 0;
    const int gap = (!icon().isNull() && showText) ? 5 : 0;
    const int contentW = iconSz.width() + gap + textW;
    const int startX = (width() - contentW) / 2;

    if (!icon().isNull()) {
      const QRect iconRect(startX, (height() - iconSz.height()) / 2,
                           iconSz.width(), iconSz.height());
      const QIcon::Mode mode =
          !isEnabled() ? QIcon::Disabled : QIcon::Normal;
      const QIcon::State iconState = checked ? QIcon::On : QIcon::Off;
      icon().paint(&p, iconRect, Qt::AlignCenter, mode, iconState);
    }
    if (showText) {
      const QRect textRect(startX + iconSz.width() + gap, 0, textW + 2,
                           height());
      p.setPen(fg);
      p.drawText(textRect, Qt::AlignVCenter | Qt::AlignLeft, text());
    }
  }
};

class Swatch : public QPushButton {
  QColor c;
  QPoint start;
  int paletteIndex = -1, historyIndex = -1;
  bool tileMode = false, tileLocked = false, historyRowMode = false;
  std::function<void(int, QString)> dropped;

  void rebuildIcon() {
    if (tileMode || historyRowMode) {
      setIcon(QIcon());
      return;
    }
    QPixmap pixmap(24, 24);
    pixmap.fill(Qt::transparent);
    QPainter painter(&pixmap);
    painter.setRenderHint(QPainter::Antialiasing, true);
    const QRect inner(1, 1, 22, 22);
    const QRectF chipRect(1.5, 1.5, 21.0, 21.0);
    QPainterPath clip;
    clip.addRoundedRect(chipRect, 3.0, 3.0);
    painter.save();
    painter.setClipPath(clip);
    if (c.alpha() < 255)
      paintCheckerboard(painter, inner, 6);
    painter.fillRect(inner, c);
    painter.restore();
    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    const QColor border =
        darkSurface ? QColor(255, 255, 255, 65) : QColor(0, 0, 0, 72);
    painter.setPen(QPen(border, 1.0));
    painter.drawRoundedRect(chipRect, 3.0, 3.0);
    setIcon(QIcon(pixmap));
    setIconSize(QSize(24, 24));
  }

public:
  Swatch(const QColor &value, const QString &name, QWidget *parent = nullptr)
      : QPushButton(name, parent), c(value) {
    setAccessibleName(name + " " + hex(c));
    setMinimumSize(64, 30);
    rebuildIcon();
  }
  void setTileMode(bool enabled, bool locked = false) {
    tileMode = enabled;
    tileLocked = locked;
    if (tileMode) {
      historyRowMode = false;
      setIcon(QIcon());
      setAttribute(Qt::WA_Hover, true);
      setCursor(Qt::PointingHandCursor);
    } else {
      rebuildIcon();
    }
    update();
  }
  void setHistoryRowMode(bool enabled) {
    historyRowMode = enabled;
    if (historyRowMode) {
      tileMode = false;
      setIcon(QIcon());
      setMinimumSize(0, 30);
      setFixedHeight(30);
      setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
      setAttribute(Qt::WA_Hover, true);
      setCursor(Qt::PointingHandCursor);
    } else {
      rebuildIcon();
    }
    updateGeometry();
    update();
  }
  QSize sizeHint() const override {
    if (historyRowMode)
      return QSize(110, 30);
    if (tileMode)
      return minimumSize().expandedTo(QSize(64, 30));
    return QPushButton::sizeHint();
  }
  QSize minimumSizeHint() const override {
    if (historyRowMode)
      return QSize(48, 30);
    if (tileMode)
      return minimumSize().expandedTo(QSize(32, 22));
    return QPushButton::minimumSizeHint();
  }
  void paletteDrop(int index, std::function<void(int, QString)> f) {
    paletteIndex = index;
    dropped = f;
    setAcceptDrops(true);
  }
  void historyDrag(int index) { historyIndex = index; }

protected:
  void changeEvent(QEvent *e) override {
    QPushButton::changeEvent(e);
    if (e->type() == QEvent::PaletteChange ||
        e->type() == QEvent::ApplicationPaletteChange)
      rebuildIcon();
  }
  void paintEvent(QPaintEvent *e) override {
    if (historyRowMode) {
      QPainter p(this);
      p.setRenderHint(QPainter::Antialiasing, true);
      const QRect r = rect();
      const QRectF pill = QRectF(r).adjusted(0.5, 0.5, -0.5, -0.5);
      const QColor fg = palette().color(
          isEnabled() ? QPalette::Active : QPalette::Disabled,
          QPalette::WindowText);
      if (isDown()) {
        QColor fill = fg;
        fill.setAlpha(24);
        p.setBrush(fill);
        p.setPen(Qt::NoPen);
        p.drawRoundedRect(pill, 3.5, 3.5);
      } else if (underMouse()) {
        QColor fill = fg;
        fill.setAlpha(14);
        p.setBrush(fill);
        p.setPen(Qt::NoPen);
        p.drawRoundedRect(pill, 3.5, 3.5);
      }
      if (hasFocus()) {
        p.setBrush(Qt::NoBrush);
        p.setPen(QPen(palette().color(QPalette::Highlight), 1.5));
        p.drawRoundedRect(pill.adjusted(0.5, 0.5, -0.5, -0.5), 3.5, 3.5);
      }
      const bool compact = r.width() < 104;
      const int padL = compact ? 3 : 5;
      const int padR = compact ? 3 : 5;
      const int chipSz = compact ? 20 : 24;
      const int gap = compact ? 4 : 6;
      const QRect chipOuter(padL, (r.height() - chipSz) / 2, chipSz, chipSz);
      const QRectF chipRect = QRectF(chipOuter).adjusted(0.5, 0.5, -0.5, -0.5);
      QPainterPath clip;
      clip.addRoundedRect(chipRect, 3.0, 3.0);
      p.save();
      p.setClipPath(clip);
      if (c.alpha() < 255)
        paintCheckerboard(p, chipOuter, 5);
      p.fillRect(chipOuter, c);
      p.restore();
      const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
      const QColor border =
          darkSurface ? QColor(255, 255, 255, 65) : QColor(0, 0, 0, 72);
      p.setPen(QPen(border, 1.0));
      p.setBrush(Qt::NoBrush);
      p.drawRoundedRect(chipRect, 3.0, 3.0);

      const int textX = chipOuter.right() + 1 + gap;
      const int textW = r.width() - textX - padR;
      if (textW > 4 && !text().isEmpty()) {
        const QRect textRect(textX, 0, textW, r.height());
        const QString elided =
            fontMetrics().elidedText(text(), Qt::ElideRight, textW);
        p.setPen(palette().color(QPalette::ButtonText));
        p.drawText(textRect, Qt::AlignLeft | Qt::AlignVCenter, elided);
      }
      return;
    }
    if (!tileMode) {
      QPushButton::paintEvent(e);
      return;
    }
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    const QRect r = rect();
    const QRectF tileRect = QRectF(r).adjusted(0.5, 0.5, -0.5, -0.5);
    const qreal radius = 4.0;
    const bool translucent = c.alpha() < 255;
    const double a = c.alphaF();
    const double cr = c.red() * a + 216.0 * (1.0 - a);
    const double cg = c.green() * a + 216.0 * (1.0 - a);
    const double cb = c.blue() * a + 216.0 * (1.0 - a);
    const double luma = 0.2126 * cr + 0.7152 * cg + 0.0722 * cb;
    const bool dark = luma < 140.0;
    const QColor fg = dark ? Qt::white : QColor(0x1B, 0x1E, 0x20);
    QPainterPath clip;
    clip.addRoundedRect(tileRect, radius, radius);
    p.save();
    p.setClipPath(clip);
    if (translucent)
      paintCheckerboard(p, r, 6);
    p.fillRect(r, c);
    if (isDown())
      p.fillRect(r, dark ? QColor(255, 255, 255, 24) : QColor(0, 0, 0, 26));
    else if (underMouse())
      p.fillRect(r, dark ? QColor(255, 255, 255, 18) : QColor(0, 0, 0, 14));
    p.restore();
    const bool compactTile = r.height() < 44;
    const int lockSz = 14;
    const QRect badgeRect(r.right() - lockSz - 5,
                          compactTile ? (r.height() - lockSz) / 2 : 5, lockSz,
                          lockSz);
    if (!text().isEmpty()) {
      QFont f = font();
      f.setBold(true);
      p.setFont(f);
      const int rightInset = tileLocked ? (r.right() - badgeRect.left() + 6) : 6;
      const QRect textRect = r.adjusted(6, 2, -rightInset, -2);
      const QString displayText =
          textRect.width() > 0
              ? p.fontMetrics().elidedText(text(), Qt::ElideRight,
                                           textRect.width())
              : QString();
      if (!displayText.isEmpty()) {
        if (translucent) {
          QRectF textBounds =
              p.boundingRect(textRect, Qt::AlignCenter, displayText);
          QRectF pill = textBounds.adjusted(-5.0, -2.0, 5.0, 2.0);
          const qreal pillRightLimit =
              tileLocked ? qMin(tileRect.right() - 3.0, badgeRect.left() - 3.0)
                         : (tileRect.right() - 3.0);
          const QRectF pillLimit(tileRect.left() + 3.0, tileRect.top() + 2.0,
                                 qMax(0.0, pillRightLimit - (tileRect.left() + 3.0)),
                                 qMax(0.0, tileRect.height() - 4.0));
          pill = pill.intersected(pillLimit);
          p.setPen(QPen(QColor(0, 0, 0, 40), 1.0));
          p.setBrush(QColor(255, 255, 255, 225));
          p.drawRoundedRect(pill, 3.0, 3.0);
          p.setPen(QColor(0x1B, 0x1E, 0x20));
          p.drawText(textRect, Qt::AlignCenter, displayText);
        } else {
          p.setPen(fg);
          p.drawText(textRect, Qt::AlignCenter, displayText);
        }
      }
    }
    if (tileLocked) {
      QIcon lockIcon =
          QIcon::fromTheme(QStringLiteral("object-locked-symbolic"));
      if (lockIcon.isNull())
        lockIcon = QIcon::fromTheme(QStringLiteral("object-locked"));
      if (!lockIcon.isNull()) {
        const qreal dpr = devicePixelRatioF();
        QPixmap pm = lockIcon.pixmap(QSize(lockSz, lockSz), dpr);
        QPixmap tinted(pm.size());
        tinted.setDevicePixelRatio(pm.devicePixelRatio());
        tinted.fill(Qt::transparent);
        QPainter tp(&tinted);
        tp.drawPixmap(0, 0, pm);
        tp.setCompositionMode(QPainter::CompositionMode_SourceIn);
        QColor badgeFg = fg;
        badgeFg.setAlpha(215);
        tp.fillRect(tinted.rect(), badgeFg);
        tp.end();
        p.drawPixmap(badgeRect.topLeft(), tinted);
      } else {
        QColor badgeFg = fg;
        badgeFg.setAlpha(215);
        p.save();
        p.setPen(QPen(badgeFg, 1.4, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
        p.setBrush(Qt::NoBrush);
        p.drawArc(QRectF(badgeRect.left() + 3.5, badgeRect.top() + 1.5, 7.0, 7.0),
                  0, 180 * 16);
        p.setBrush(badgeFg);
        p.setPen(Qt::NoPen);
        p.drawRoundedRect(
            QRectF(badgeRect.left() + 2.0, badgeRect.top() + 6.0, 10.0, 7.0),
            1.5, 1.5);
        p.restore();
      }
    }
    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    const QColor frameColor =
        darkSurface ? QColor(255, 255, 255, 65) : QColor(0, 0, 0, 72);
    p.setPen(QPen(frameColor, 1.0));
    p.setBrush(Qt::NoBrush);
    p.drawRoundedRect(tileRect, radius, radius);
    if (hasFocus()) {
      QPen focusPen(text().isEmpty() ? palette().color(QPalette::Highlight) : fg,
                    2.0);
      p.setPen(focusPen);
      p.setBrush(Qt::NoBrush);
      p.drawRoundedRect(tileRect.adjusted(1.0, 1.0, -1.0, -1.0), radius - 1.0,
                        radius - 1.0);
    } else if (underMouse()) {
      QColor hoverBorder = fg;
      hoverBorder.setAlpha(105);
      p.setPen(QPen(hoverBorder, 1.0));
      p.setBrush(Qt::NoBrush);
      p.drawRoundedRect(tileRect, radius, radius);
    }
  }
  void mousePressEvent(QMouseEvent *e) override {
    start = e->pos();
    QPushButton::mousePressEvent(e);
  }
  void mouseMoveEvent(QMouseEvent *e) override {
    if ((e->buttons() & Qt::LeftButton) &&
        (e->pos() - start).manhattanLength() >=
            QApplication::startDragDistance()) {
      auto d = new QDrag(this);
      auto m = new QMimeData;
      m->setText(hex(c));
      if (paletteIndex >= 0)
        m->setData("application/x-swatch-board-index",
                   QByteArray::number(paletteIndex));
      if (historyIndex >= 0)
        m->setData("application/x-swatch-board-history",
                   QByteArray::number(historyIndex));
      d->setMimeData(m);
      d->exec(Qt::CopyAction);
    }
  }
  void dragEnterEvent(QDragEnterEvent *e) override {
    if (dropped && e->mimeData()->hasText())
      e->acceptProposedAction();
  }
  void dropEvent(QDropEvent *e) override {
    if (!dropped)
      return;
    bool ok = false;
    int index =
        e->mimeData()->data("application/x-swatch-board-index").toInt(&ok);
    if (!ok) {
      index =
          e->mimeData()->data("application/x-swatch-board-history").toInt(&ok);
      index = ok ? -index - 2 : -1;
    }
    dropped(index, e->mimeData()->text());
    e->acceptProposedAction();
  }
};
class CurrentColorBanner : public QWidget {
  std::function<void(QString)> dropped;
  QHBoxLayout *rowLayout = nullptr;
  QWidget *chipWidget = nullptr;
  QLabel *hexLabel = nullptr;
  QLabel *sepLabel = nullptr;
  QLabel *outLabel = nullptr;
  QLabel *hintLabel = nullptr;
  QString fullOutputText;
  bool dragHover = false;

  void updateLayoutMetrics() {
    if (!rowLayout || !chipWidget || !hexLabel)
      return;
    const auto m = rowLayout->contentsMargins();
    const int sp = rowLayout->spacing();
    const int hexW = hexLabel->fontMetrics().horizontalAdvance(hexLabel->text()) + 4;
    int leftFixed = m.left() + m.right() + chipWidget->width() + sp + hexW;
    if (sepLabel)
      leftFixed += sp + sepLabel->fontMetrics().horizontalAdvance(sepLabel->text()) + 2;
    const int fullOutW =
        outLabel ? outLabel->fontMetrics().horizontalAdvance(fullOutputText) + 4 : 0;
    const int hintW =
        hintLabel ? hintLabel->fontMetrics().horizontalAdvance(hintLabel->text()) + 4 : 0;
    const int neededWithHint =
        leftFixed + (outLabel ? sp + fullOutW : 0) + (hintLabel ? sp + hintW : 0) + 12;
    const bool showHint = hintLabel && width() >= neededWithHint;
    if (hintLabel)
      hintLabel->setVisible(showHint);
    if (outLabel) {
      const int availOutW = width() - leftFixed - sp - (showHint ? (sp + hintW) : 0);
      if (availOutW <= 4) {
        outLabel->setText(QString());
      } else {
        outLabel->setText(
            outLabel->fontMetrics().elidedText(fullOutputText, Qt::ElideRight, availOutW));
      }
    }
  }

public:
  CurrentColorBanner(const QColor &value, const QString &currentHex,
                     const QString &outputText, int headerHeight,
                     std::function<void(QString)> onDrop,
                     QWidget *parent = nullptr)
      : QWidget(parent), dropped(std::move(onDrop)) {
    setAcceptDrops(true);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    auto outer = new QVBoxLayout(this);
    outer->setContentsMargins(0, 0, 0, 0);
    outer->setSpacing(0);

    auto rowWidget = new QWidget(this);
    rowWidget->setObjectName("currentColorRow");
    rowWidget->setFixedHeight(headerHeight);
    rowWidget->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    rowLayout = new QHBoxLayout(rowWidget);
    rowLayout->setContentsMargins(10, 0, 10, 0);
    rowLayout->setSpacing(10);

    auto chip = new Swatch(value, "Current color", rowWidget);
    chip->setText(QString());
    chip->setFixedSize(48, 22);
    chip->setTileMode(true, false);
    chip->setToolTip(
        "Current color — drag to palette or theme, or drop a color here");
    chip->paletteDrop(-1, [this](int, QString c) {
      if (dropped && color(c).isValid())
        dropped(c);
    });
    chipWidget = chip;
    rowLayout->addWidget(chip, 0, Qt::AlignVCenter);

    const QFont monoFont = QFontDatabase::systemFont(QFontDatabase::FixedFont);
    hexLabel = new QLabel(currentHex, rowWidget);
    QFont hexFont = monoFont;
    hexFont.setBold(true);
    hexLabel->setFont(hexFont);
    hexLabel->setTextInteractionFlags(Qt::TextSelectableByMouse);
    rowLayout->addWidget(hexLabel, 0, Qt::AlignVCenter);

    auto makeMuted = [rowWidget](const QString &text, int alpha, bool smaller = false) {
      auto l = new QLabel(text, rowWidget);
      if (smaller) {
        QFont f = l->font();
        if (f.pointSizeF() > 0)
          f.setPointSizeF(qMax(8.5, f.pointSizeF() - 1.0));
        else if (f.pixelSize() > 0)
          f.setPixelSize(qMax(10, f.pixelSize() - 1));
        l->setFont(f);
      }
      QPalette pal = l->palette();
      const QColor muted = mutedColor(pal, alpha);
      pal.setColor(QPalette::WindowText, muted);
      pal.setColor(QPalette::Text, muted);
      l->setPalette(pal);
      return l;
    };

    if (!outputText.isEmpty()) {
      sepLabel = makeMuted(QString::fromUtf8("·"), 120);
      rowLayout->addWidget(sepLabel, 0, Qt::AlignVCenter);
      fullOutputText = "Output: " + outputText;
      outLabel = makeMuted(fullOutputText, 175);
      outLabel->setToolTip(fullOutputText);
      outLabel->setTextInteractionFlags(Qt::TextSelectableByMouse);
      outLabel->setMinimumWidth(0);
      outLabel->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
      rowLayout->addWidget(outLabel, 1, Qt::AlignVCenter);
    } else {
      rowLayout->addStretch(1);
    }

    hintLabel = makeMuted(
        QString::fromUtf8("Drag swatch to palette or theme · Drop color here"),
        130, true);
    hintLabel->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    hintLabel->setMinimumWidth(0);
    rowLayout->addWidget(hintLabel, 0, Qt::AlignVCenter);

    outer->addWidget(rowWidget);
    outer->addWidget(new Hairline(this));
    updateLayoutMetrics();
  }
  QSize minimumSizeHint() const override {
    return QSize(0, QWidget::minimumSizeHint().height());
  }

protected:
  void showEvent(QShowEvent *e) override {
    QWidget::showEvent(e);
    updateLayoutMetrics();
  }
  void resizeEvent(QResizeEvent *e) override {
    QWidget::resizeEvent(e);
    updateLayoutMetrics();
  }
  void paintEvent(QPaintEvent *e) override {
    QWidget::paintEvent(e);
    if (dragHover) {
      QPainter p(this);
      QColor fill = palette().color(QPalette::Highlight);
      fill.setAlpha(24);
      p.fillRect(rect(), fill);
    }
  }
  void dragEnterEvent(QDragEnterEvent *e) override {
    if (dropped && e->mimeData()->hasText() &&
        color(e->mimeData()->text()).isValid()) {
      dragHover = true;
      update();
      e->acceptProposedAction();
    }
  }
  void dragLeaveEvent(QDragLeaveEvent *) override {
    dragHover = false;
    update();
  }
  void dropEvent(QDropEvent *e) override {
    dragHover = false;
    update();
    if (dropped && e->mimeData()->hasText() &&
        color(e->mimeData()->text()).isValid()) {
      dropped(e->mimeData()->text());
      e->acceptProposedAction();
    }
  }
};
// A dropped swatch replaces the complete color value rather than being inserted
// at the cursor position as QLineEdit's default drop handling does.
class ColorLineEdit : public QLineEdit {
public:
  ColorLineEdit(const QString &value, QWidget *parent) : QLineEdit(value, parent) {
    setAccessibleName("Role color");
    setAcceptDrops(true);
  }
protected:
  void dragEnterEvent(QDragEnterEvent *e) override {
    if (e->mimeData()->hasText() && color(e->mimeData()->text()).isValid())
      e->acceptProposedAction();
  }
  void dropEvent(QDropEvent *e) override {
    if (e->mimeData()->hasText() && color(e->mimeData()->text()).isValid()) {
      setText(e->mimeData()->text());
      emit editingFinished();
      e->acceptProposedAction();
    }
  }
};
class PaletteDrop : public QLabel {
  std::function<void(int, QString)> append;
  bool dragHover = false;

public:
  PaletteDrop(std::function<void(int, QString)> f, QWidget *parent)
      : QLabel("Drop a history color here to add it", parent), append(f) {
    setAcceptDrops(true);
    setAccessibleName("Drop a history color to add it to the palette");
    setAlignment(Qt::AlignCenter);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    setContentsMargins(12, 0, 12, 0);
    setMinimumHeight(fontMetrics().height() + 2);
    QPalette pal = palette();
    const QColor muted = mutedColor(pal, 150);
    pal.setColor(QPalette::WindowText, muted);
    pal.setColor(QPalette::Text, muted);
    setPalette(pal);
  }

protected:
  void paintEvent(QPaintEvent *e) override {
    const bool compactDrop = height() <= fontMetrics().height() + 10;
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    const QRectF zone = compactDrop
                            ? QRectF(rect()).adjusted(6.5, 2.5, -6.5, -2.5)
                            : QRectF(rect()).adjusted(6.5, 6.5, -6.5, -6.5);
    const qreal radius = compactDrop ? 3.5 : 5.0;
    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    QColor fill = dragHover ? palette().color(QPalette::Highlight)
                            : palette().color(QPalette::WindowText);
    fill.setAlpha(dragHover ? 24 : 8);
    QColor border = dragHover
                        ? palette().color(QPalette::Highlight)
                        : (darkSurface ? QColor(255, 255, 255, compactDrop ? 90 : 105)
                                       : QColor(0, 0, 0, compactDrop ? 80 : 95));
    if (dragHover)
      border.setAlpha(215);
    p.setBrush(fill);
    p.setPen(QPen(border, 1.0, Qt::DashLine));
    p.drawRoundedRect(zone, radius, radius);
    p.end();
    QLabel::paintEvent(e);
  }
  void dragEnterEvent(QDragEnterEvent *e) override {
    if (e->mimeData()->hasText() &&
        !e->mimeData()->hasFormat("application/x-swatch-board-index")) {
      dragHover = true;
      update();
      e->acceptProposedAction();
    }
  }
  void dragLeaveEvent(QDragLeaveEvent *) override {
    dragHover = false;
    update();
  }
  void dropEvent(QDropEvent *e) override {
    dragHover = false;
    update();
    auto c = e->mimeData()->text();
    bool ok = false;
    int index =
        e->mimeData()->data("application/x-swatch-board-history").toInt(&ok);
    if (color(c).isValid()) {
      append(ok ? index : -1, c);
      e->acceptProposedAction();
    }
  }
};
class PopupFrame : public QWidget {
public:
  explicit PopupFrame(QWidget *parent = nullptr) : QWidget(parent) {
    setAutoFillBackground(false);
  }

protected:
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    p.fillRect(rect(), palette().color(QPalette::Window));
    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    const QColor border =
        darkSurface ? QColor(255, 255, 255, 65) : QColor(0, 0, 0, 72);
    p.setPen(QPen(border, 1.0));
    p.setBrush(Qt::NoBrush);
    p.drawRect(QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5));
  }
};
class PreviewStrip : public QWidget {
  QList<QColor> stripColors;
  QStringList stripLabels;

  QString labelFor(int index) const {
    const QString raw = stripLabels.value(index);
    return raw.isEmpty() ? hex(stripColors[index]) : raw.toUpper();
  }

public:
  explicit PreviewStrip(QWidget *parent = nullptr) : QWidget(parent) {
    setAccessibleName("Generated palette preview");
    setFixedHeight(38);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    setMouseTracking(true);
  }
  void setColors(const QList<QColor> &colors, const QStringList &labels = {}) {
    stripColors = colors;
    stripLabels = labels;
    update();
  }
  int colorCount() const { return stripColors.size(); }

protected:
  bool event(QEvent *e) override {
    if (e->type() == QEvent::ToolTip) {
      auto *help = static_cast<QHelpEvent *>(e);
      if (!stripColors.isEmpty() && width() > 0) {
        const int idx = qBound(
            0, int(help->pos().x() * stripColors.size() / width()),
            int(stripColors.size()) - 1);
        QToolTip::showText(help->globalPos(), labelFor(idx), this);
      } else {
        QToolTip::hideText();
        e->ignore();
      }
      return true;
    }
    return QWidget::event(e);
  }
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    const QRect r = rect();
    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;

    if (stripColors.isEmpty()) {
      const QRectF frameRect = QRectF(r).adjusted(0.5, 0.5, -0.5, -0.5);
      const qreal radius = 4.0;
      QPainterPath clip;
      clip.addRoundedRect(frameRect, radius, radius);
      QColor border = darkSurface ? QColor(255, 255, 255, 65)
                                  : palette().color(QPalette::Mid);
      if (!darkSurface)
        border.setAlpha(180);
      p.save();
      p.setClipPath(clip);
      p.fillRect(r, palette().color(QPalette::Base));
      p.setPen(mutedColor(palette(), 145));
      p.drawText(r, Qt::AlignCenter, QStringLiteral("No preview"));
      p.restore();
      p.setPen(QPen(border, 1.0));
      p.setBrush(Qt::NoBrush);
      p.drawRoundedRect(frameRect, radius, radius);
      return;
    }

    const int n = stripColors.size();
    const int gap = (n > 1 && r.width() >= n * 8) ? 3 : 0;
    const double availW = qMax<double>(n, r.width() - gap * (n - 1));
    auto tileIntRect = [&](int i) {
      const int x0 = qRound(i * availW / n) + i * gap;
      const int x1 = (i == n - 1) ? r.width() : qRound((i + 1) * availW / n) + i * gap;
      return QRect(x0, r.top(), qMax(1, x1 - x0), r.height());
    };

    QFont labelFont = font();
    labelFont.setBold(true);
    if (labelFont.pointSizeF() > 0)
      labelFont.setPointSizeF(qMax(8.0, labelFont.pointSizeF() - 1.0));
    else if (labelFont.pixelSize() > 0)
      labelFont.setPixelSize(qMax(9, labelFont.pixelSize() - 1));
    const QFontMetrics fm(labelFont);

    bool showFull = true;
    bool showShort = true;
    for (int i = 0; i < n; ++i) {
      const int segW = tileIntRect(i).width();
      const QString full = labelFor(i);
      if (segW < fm.horizontalAdvance(full) + 8)
        showFull = false;
      const QString shortLabel =
          (stripColors[i].alpha() == 255 && full.size() == 9) ? full.left(7)
                                                               : full;
      if (segW < fm.horizontalAdvance(shortLabel) + 8)
        showShort = false;
    }

    const QColor tileBorder =
        darkSurface ? QColor(255, 255, 255, 65) : QColor(0, 0, 0, 72);
    p.setFont(labelFont);
    for (int i = 0; i < n; ++i) {
      const QRect segRect = tileIntRect(i);
      const QRectF tileRect = QRectF(segRect).adjusted(0.5, 0.5, -0.5, -0.5);
      const qreal tileRadius = qMin<qreal>(3.5, tileRect.width() / 2.0);
      const QColor &c = stripColors[i];
      const bool translucent = c.alpha() < 255;

      QPainterPath tileClip;
      tileClip.addRoundedRect(tileRect, tileRadius, tileRadius);
      p.save();
      p.setClipPath(tileClip);
      if (translucent)
        paintCheckerboard(p, segRect, 6);
      p.fillRect(segRect, c);

      if (showFull || showShort) {
        const QString full = labelFor(i);
        const QString text =
            showFull ? full
                     : ((c.alpha() == 255 && full.size() == 9) ? full.left(7)
                                                               : full);
        const double a = c.alphaF();
        const double cr = c.red() * a + 216.0 * (1.0 - a);
        const double cg = c.green() * a + 216.0 * (1.0 - a);
        const double cb = c.blue() * a + 216.0 * (1.0 - a);
        const double luma = 0.2126 * cr + 0.7152 * cg + 0.0722 * cb;
        const QColor fg = luma < 140.0 ? Qt::white : QColor(0x1B, 0x1E, 0x20);
        const QRect textRect = segRect.adjusted(3, 2, -3, -2);
        if (translucent) {
          QRectF bounds = p.boundingRect(textRect, Qt::AlignCenter, text);
          QRectF pill = bounds.adjusted(-4.0, -1.5, 4.0, 1.5)
                            .intersected(tileRect.adjusted(2.0, 2.0, -2.0, -2.0));
          p.setPen(QPen(QColor(0, 0, 0, 40), 1.0));
          p.setBrush(QColor(255, 255, 255, 225));
          p.drawRoundedRect(pill, 3.0, 3.0);
          p.setPen(QColor(0x1B, 0x1E, 0x20));
          p.drawText(textRect, Qt::AlignCenter, text);
        } else {
          p.setPen(fg);
          p.drawText(textRect, Qt::AlignCenter, text);
        }
      }
      p.restore();

      p.setPen(QPen(tileBorder, 1.0));
      p.setBrush(Qt::NoBrush);
      p.drawRoundedRect(tileRect, tileRadius, tileRadius);
    }
  }
};
class ColorChip : public QWidget {
  QColor chipColor;

  void updateToolTip() {
    setToolTip(chipColor.isValid()
                   ? QStringLiteral("Base color: %1").arg(hex(chipColor))
                   : QStringLiteral("Invalid base color"));
  }

public:
  explicit ColorChip(const QColor &initial = {}, QWidget *parent = nullptr)
      : QWidget(parent), chipColor(initial) {
    setFixedSize(24, 24);
    setAccessibleName("Base color preview");
    updateToolTip();
  }
  void setColor(const QColor &value) {
    if (chipColor != value) {
      chipColor = value;
      updateToolTip();
      update();
    }
  }

protected:
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    const QRect r = rect();
    const QRectF frame = QRectF(r).adjusted(0.5, 0.5, -0.5, -0.5);
    const qreal radius = 3.5;
    QPainterPath clip;
    clip.addRoundedRect(frame, radius, radius);
    p.save();
    p.setClipPath(clip);
    if (chipColor.isValid()) {
      if (chipColor.alpha() < 255)
        paintCheckerboard(p, r, 6);
      p.fillRect(r, chipColor);
    } else {
      p.fillRect(r, palette().color(QPalette::Base));
      p.setPen(QPen(mutedColor(palette(), 145), 1.2));
      p.drawLine(r.topLeft() + QPoint(5, 5), r.bottomRight() - QPoint(5, 5));
      p.drawLine(r.topRight() + QPoint(-5, 5), r.bottomLeft() + QPoint(5, -5));
    }
    p.restore();
    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    const QColor border =
        darkSurface ? QColor(255, 255, 255, 65) : QColor(0, 0, 0, 72);
    p.setPen(QPen(border, 1.0));
    p.setBrush(Qt::NoBrush);
    p.drawRoundedRect(frame, radius, radius);
  }
};
class PrimaryActionButton : public QPushButton {
  bool updatingStyle = false;
  bool primary = true;
  QFont baseFont;

  void applyStyle() {
    if (updatingStyle)
      return;
    updatingStyle = true;
    if (!primary) {
      setFont(baseFont);
      setStyleSheet(QString());
      updatingStyle = false;
      return;
    }
    QFont f = baseFont;
    f.setWeight(QFont::DemiBold);
    setFont(f);
    const QPalette basePal = QApplication::palette("QPushButton");
    const QColor hi = basePal.color(QPalette::Active, QPalette::Highlight);
    const QColor fg = basePal.color(QPalette::Active, QPalette::ButtonText);
    const auto rgba = [](const QColor &c, int a) {
      return QString("rgba(%1, %2, %3, %4)")
          .arg(c.red())
          .arg(c.green())
          .arg(c.blue())
          .arg(a);
    };
    setStyleSheet(QString(
                      "QPushButton {"
                      "  background-color: %1;"
                      "  border: 1px solid %2;"
                      "  border-radius: 4px;"
                      "  padding: 5px 18px;"
                      "  color: %3;"
                      "}"
                      "QPushButton:hover {"
                      "  background-color: %4;"
                      "  border: 1px solid %5;"
                      "}"
                      "QPushButton:pressed {"
                      "  background-color: %6;"
                      "  border: 1px solid %7;"
                      "}"
                      "QPushButton:focus {"
                      "  border: 1px solid %7;"
                      "}")
                      .arg(rgba(hi, 34), rgba(hi, 175), fg.name(),
                           rgba(hi, 52), rgba(hi, 210),
                           rgba(hi, 74), rgba(hi, 235)));
    updatingStyle = false;
  }

public:
  explicit PrimaryActionButton(const QString &label, QWidget *parent = nullptr)
      : QPushButton(label, parent), baseFont(font()) {
    setAccessibleName(label);
    applyStyle();
  }
  void setPrimary(bool active) {
    if (primary != active) {
      primary = active;
      applyStyle();
    }
  }

protected:
  void changeEvent(QEvent *e) override {
    QPushButton::changeEvent(e);
    if (!updatingStyle &&
        (e->type() == QEvent::PaletteChange ||
         e->type() == QEvent::ApplicationPaletteChange))
      applyStyle();
  }
};
class Spectrum : public QWidget {
  double h, s, v;
  std::function<void(double, double, double, bool)> changed;
  void choose(QPoint p, bool final) {
    s = qBound(0.0, p.x() * 100.0 / qMax(1, width() - 1), 100.0);
    v = qBound(0.0, 100.0 - p.y() * 100.0 / qMax(1, height() - 1), 100.0);
    update();
    changed(h, s, v, final);
  }

public:
  Spectrum(double hue, double sat, double val,
           std::function<void(double, double, double, bool)> f, QWidget *parent)
      : QWidget(parent), h(hue), s(sat), v(val), changed(f) {
    setFixedSize(156, 156);
    setFocusPolicy(Qt::StrongFocus);
    setCursor(Qt::CrossCursor);
    setAccessibleName("Saturation and value spectrum; arrow keys adjust");
  }
  void setHsv(double hue, double sat, double val) {
    h = hue; s = sat; v = val; update();
  }
  double saturation() const { return s; }
  double hue() const { return h; }

protected:
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    const QRectF bounds = QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5);
    const qreal radius = 3.0;
    QPainterPath clip;
    clip.addRoundedRect(bounds, radius, radius);
    p.save();
    p.setClipPath(clip);
    const double hueNorm = std::fmod(std::fmod(h, 360.0) + 360.0, 360.0) / 360.0;
    QLinearGradient a(0, 0, width(), 0);
    a.setColorAt(0, Qt::white);
    a.setColorAt(1, QColor::fromHsvF(hueNorm, 1.0, 1.0));
    p.fillRect(rect(), a);
    QLinearGradient b(0, 0, 0, height());
    b.setColorAt(0, Qt::transparent);
    b.setColorAt(1, Qt::black);
    p.fillRect(rect(), b);
    p.restore();

    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    const QColor border =
        darkSurface ? QColor(255, 255, 255, 65) : QColor(0, 0, 0, 68);
    p.setPen(QPen(border, 1.0));
    p.setBrush(Qt::NoBrush);
    p.drawRoundedRect(bounds, radius, radius);

    const qreal pad = 7.5;
    const qreal rawX = s * (width() - 1) / 100.0;
    const qreal rawY = (100.0 - v) * (height() - 1) / 100.0;
    const QPointF center(
        qBound(pad, rawX, qMax(pad, width() - pad)),
        qBound(pad, rawY, qMax(pad, height() - pad)));
    p.setBrush(Qt::NoBrush);
    p.setPen(QPen(QColor(0, 0, 0, 225), 1.5));
    p.drawEllipse(center, 6.2, 6.2);
    p.setPen(QPen(Qt::white, 1.6));
    p.drawEllipse(center, 5.0, 5.0);
    p.setPen(QPen(QColor(0, 0, 0, 190), 1.0));
    p.drawEllipse(center, 3.8, 3.8);

    if (hasFocus()) {
      p.setPen(QPen(palette().color(QPalette::Highlight), 2.0));
      p.setBrush(Qt::NoBrush);
      p.drawRoundedRect(QRectF(rect()).adjusted(1.0, 1.0, -1.0, -1.0), radius,
                        radius);
    }
  }
  void mousePressEvent(QMouseEvent *e) override {
    if (e->button() == Qt::LeftButton) {
      setFocus(Qt::MouseFocusReason);
      choose(e->pos(), false);
    }
  }
  void mouseMoveEvent(QMouseEvent *e) override {
    if (e->buttons() & Qt::LeftButton)
      choose(e->pos(), false);
  }
  void mouseReleaseEvent(QMouseEvent *e) override {
    if (e->button() == Qt::LeftButton) choose(e->pos(), true);
  }
  void keyPressEvent(QKeyEvent *e) override {
    int ds = 0, dv = 0;
    if (e->key() == Qt::Key_Left)
      ds = -1;
    else if (e->key() == Qt::Key_Right)
      ds = 1;
    else if (e->key() == Qt::Key_Up)
      dv = 1;
    else if (e->key() == Qt::Key_Down)
      dv = -1;
    else {
      QWidget::keyPressEvent(e);
      return;
    }
    s = qBound(0.0, s + ds, 100.0);
    v = qBound(0.0, v + dv, 100.0);
    update();
    changed(h, s, v, true);
  }
};
// The mixer wheel selects hue and saturation; value and alpha stay in the model.
class MixerWheel : public QWidget {
  double h, s, v;
  std::function<void(double, double, double, bool)> changed;
  QRectF wheelRect() const {
    const double side = qMax(1, qMin(width(), height()) - 16);
    return QRectF((width() - side) / 2, (height() - side) / 2, side, side);
  }
  void choose(QPointF point, bool final) {
    const auto r = wheelRect();
    const auto d = point - r.center();
    h = std::atan2(-d.y(), d.x()) * 180.0 / M_PI;
    if (h < 0) h += 360;
    s = qMin(100.0, std::hypot(d.x(), d.y()) * 200.0 / r.width());
    update();
    changed(h, s, v, final);
  }
public:
  MixerWheel(double hue, double sat, double val,
             std::function<void(double, double, double, bool)> callback,
             QWidget *parent)
      : QWidget(parent), h(hue), s(sat), v(val), changed(callback) {
    setFixedSize(156, 156);
    setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Preferred);
    setFocusPolicy(Qt::StrongFocus);
    setCursor(Qt::CrossCursor);
    setAccessibleName("Mixer color wheel; arrow keys adjust hue and saturation");
  }
  void setHsv(double hue, double sat, double val) {
    h = hue; s = sat; v = val; update();
  }
  double hue() const { return h; }
  double saturation() const { return s; }
protected:
  void paintEvent(QPaintEvent *) override {
    const auto r = wheelRect();
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
          const double edgeAlpha =
              qBound(0.0, (radius + 0.5 - dist) / aaBand, 1.0);
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
    p.setPen(QPen(darkSurface ? QColor(255, 255, 255, 58) : QColor(0, 0, 0, 56),
                  1.0));
    p.setBrush(Qt::NoBrush);
    p.drawEllipse(r.adjusted(0.5, 0.5, -0.5, -0.5));
    const double angle = h * M_PI / 180.0;
    const QPointF marker =
        r.center() + QPointF(std::cos(angle), -std::sin(angle)) *
                         (s * r.width() / 200.0);
    p.setBrush(Qt::NoBrush);
    p.setPen(QPen(QColor(0, 0, 0, 225), 1.5));
    p.drawEllipse(marker, 6.2, 6.2);
    p.setPen(QPen(Qt::white, 1.6));
    p.drawEllipse(marker, 5.0, 5.0);
    p.setPen(QPen(QColor(0, 0, 0, 190), 1.0));
    p.drawEllipse(marker, 3.8, 3.8);
    if (hasFocus()) {
      p.setPen(QPen(palette().color(QPalette::Highlight), 2.0));
      p.drawRoundedRect(QRectF(rect()).adjusted(1.0, 1.0, -1.0, -1.0), 3.0,
                        3.0);
    }
  }
  void mousePressEvent(QMouseEvent *e) override {
    if (e->button() == Qt::LeftButton) { setFocus(Qt::MouseFocusReason); choose(e->position(), false); }
  }
  void mouseMoveEvent(QMouseEvent *e) override {
    if (e->buttons() & Qt::LeftButton) choose(e->position(), false);
  }
  void mouseReleaseEvent(QMouseEvent *e) override {
    if (e->button() == Qt::LeftButton) choose(e->position(), true);
  }
  void keyPressEvent(QKeyEvent *e) override {
    if (e->key() == Qt::Key_Left) h = std::fmod(h + 359.0, 360.0);
    else if (e->key() == Qt::Key_Right) h = std::fmod(h + 1.0, 360.0);
    else if (e->key() == Qt::Key_Up) s = qMin(100.0, s + 1);
    else if (e->key() == Qt::Key_Down) s = qMax(0.0, s - 1);
    else { QWidget::keyPressEvent(e); return; }
    update(); changed(h, s, v, true);
  }
};
class Scene : public QWidget {
  QJsonObject scene;
  QVector<QImage> images;
  std::function<void(QJsonObject)> send;
  bool topAligned = false;
  bool compactMode = false;
  QString selectedRole;
  QString selectedRoleTitle;
  int hoveredIndex = -1;
  bool dragHover = false;

public:
  QRectF targetRect() const {
    double w = scene.value("width").toDouble(1),
           h = scene.value("height").toDouble(1);
    if (w <= 0 || h <= 0)
      return {};
    if (compactMode)
      return QRectF(0, 0, width(), height());
    double scale = qMin(width() / w, height() / h);
    return QRectF((width() - w * scale) / 2, topAligned ? 0 : (height() - h * scale) / 2,
                  w * scale, h * scale);
  }

private:
  int hitIndex(QPoint point) const {
    auto r = targetRect();
    if (!r.contains(point))
      return -1;
    auto elements = array(scene, "elements");
    double x = (point.x() - r.x()) * scene.value("width").toDouble() /
               r.width(),
           y = (point.y() - r.y()) * scene.value("height").toDouble() /
               r.height();
    for (int i = elements.size() - 1; i >= 0; --i) {
      auto e = elements[i].toObject();
      auto a = array(e, "rect");
      if (a.size() == 4 && e.value("enabled").toBool() &&
          QRectF(a[0].toDouble(), a[1].toDouble(), a[2].toDouble(),
                 a[3].toDouble())
              .contains(x, y))
        return i;
    }
    return -1;
  }
  QJsonObject hit(QPoint point) const {
    const int idx = hitIndex(point);
    if (idx < 0)
      return {};
    return array(scene, "elements")[idx].toObject();
  }
  QRectF elementWidgetRect(const QJsonObject &e, const QRectF &r) const {
    auto a = array(e, "rect");
    if (a.size() != 4)
      return {};
    const double sw = scene.value("width").toDouble(1);
    const double sh = scene.value("height").toDouble(1);
    if (sw <= 0 || sh <= 0)
      return {};
    const double sx = r.width() / sw;
    const double sy = r.height() / sh;
    return QRectF(r.left() + a[0].toDouble() * sx,
                  r.top() + a[1].toDouble() * sy,
                  a[2].toDouble() * sx,
                  a[3].toDouble() * sy);
  }

public:
  Scene(QJsonObject data, std::function<void(QJsonObject)> f, QWidget *parent)
      : QWidget(parent), scene(data), send(f) {
    setMinimumSize(140, 68);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    setAcceptDrops(true);
    setMouseTracking(true);
    setAccessibleName("Theme scene preview");
    setToolTip("Click an element to select its role, or drop a color to assign it");
    setFocusPolicy(Qt::StrongFocus);
    for (auto value : array(scene, "elements")) {
      auto bytes = array(value.toObject(), "asset");
      QByteArray data;
      data.reserve(bytes.size());
      for (auto byte : bytes)
        data.append(char(byte.toInt()));
      QImage image;
      if (!data.isEmpty())
        image.loadFromData(data, "PNG");
      images.append(image);
    }
  }
  int wideTargetHeight(int forWidth) const {
    const double sw = scene.value("width").toDouble(0);
    const double sh = scene.value("height").toDouble(0);
    if (sw <= 0 || sh <= 0 || forWidth <= 0)
      return 220;
    return qBound(160, qRound(forWidth * sh / sw) + 2, 240);
  }
  void syncWideMaxHeight(int forWidth = -1) {
    if (compactMode)
      return;
    const int w = forWidth > 0 ? forWidth : width();
    if (w <= 0)
      return;
    const int targetH = wideTargetHeight(w);
    if (maximumHeight() != targetH) {
      setMaximumHeight(targetH);
      updateGeometry();
    }
  }
  QSize sizeHint() const override {
    if (compactMode)
      return QSize(420, 78);
    const int w = width() > 140 ? width() : 390;
    return QSize(390, wideTargetHeight(w));
  }
  void setTopAligned(bool aligned) {
    topAligned = aligned;
    update();
  }
  void setCompactMode(bool compact) {
    if (compactMode != compact) {
      compactMode = compact;
      setMinimumSize(140, compact ? 64 : 68);
      updateGeometry();
      update();
    }
  }
  void setSelectedRole(const QString &role, const QString &title = QString()) {
    if (selectedRole != role || selectedRoleTitle != title) {
      selectedRole = role;
      selectedRoleTitle = title;
      update();
    }
  }

protected:
  void resizeEvent(QResizeEvent *e) override {
    QWidget::resizeEvent(e);
    if (!compactMode && e->size().width() > 0)
      syncWideMaxHeight(e->size().width());
  }
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    p.setRenderHint(QPainter::TextAntialiasing, true);
    p.setRenderHint(QPainter::SmoothPixmapTransform, true);
    const QRectF r = targetRect();
    if (r.isEmpty())
      return;
    const QRectF frame = r.adjusted(0.5, 0.5, -0.5, -0.5);
    const qreal radius = compactMode ? 4.0 : 5.0;
    QPainterPath clip;
    clip.addRoundedRect(frame, radius, radius);
    const double sceneW = scene.value("width").toDouble(1);
    const double sceneH = scene.value("height").toDouble(1);
    const double scaleX = r.width() / qMax(1.0, sceneW);
    const double scaleY = r.height() / qMax(1.0, sceneH);
    const double uniformScale = qMin(scaleX, scaleY);

    auto elements = array(scene, "elements");
    QColor baseBg = palette().color(QPalette::Window);
    if (!elements.isEmpty()) {
      const QColor c0 = color(str(elements[0].toObject(), "fill"));
      if (c0.isValid() && c0.alpha() > 0)
        baseBg = c0;
    }

    p.save();
    p.setClipPath(clip);
    bool backgroundSelected = false;
    QVector<QRectF> selectedBoxes;
    for (int i = 0; i < elements.size(); ++i) {
      auto e = elements[i].toObject();
      auto a = array(e, "rect");
      if (a.size() != 4)
        continue;
      const QRectF box = elementWidgetRect(e, r);
      if (box.isEmpty())
        continue;
      const auto fill = color(str(e, "fill"));
      const bool hasFill = fill.isValid() && fill.alpha() > 0;
      const qreal elemRadius = (i > 0) ? (compactMode ? 3.0 : 4.0) : 0.0;
      if (hasFill) {
        if (elemRadius > 0.0) {
          p.setPen(Qt::NoPen);
          p.setBrush(fill);
          p.drawRoundedRect(box, elemRadius, elemRadius);
        } else {
          p.fillRect(box, fill);
        }
      }
      const auto elemBorder = color(str(e, "border"));
      if (elemBorder.isValid() && elemBorder.alpha() > 0) {
        p.setBrush(Qt::NoBrush);
        p.setPen(QPen(elemBorder, 1.0));
        if (elemRadius > 0.0)
          p.drawRoundedRect(box.adjusted(0.5, 0.5, -0.5, -0.5), elemRadius,
                            elemRadius);
        else
          p.drawRect(box.adjusted(0.5, 0.5, -0.5, -0.5));
      }
      if (i < images.size() && !images[i].isNull())
        p.drawImage(box, images[i]);

      const auto text = str(e, "text");
      if (!text.isEmpty()) {
        p.setPen(color(str(e, "text_color")));
        const QString trimmed = text.trimmed();
        const bool isCodeText = trimmed.startsWith('$') || trimmed.startsWith('>') ||
                                trimmed.contains('=') || str(e, "kind") == "syntax";
        QFont f = isCodeText
                      ? QFontDatabase::systemFont(QFontDatabase::FixedFont)
                      : font();
        f.setStyleStrategy(QFont::PreferAntialias);
        const double explicitPx = e.value("font_size").toDouble(0);
        if (explicitPx > 0) {
          f.setPixelSize(qMax(9, qRound(explicitPx * uniformScale)));
        } else if (isCodeText) {
          const int targetPx = compactMode
                                   ? qBound(9, qRound(box.height() * 0.62), 12)
                                   : qBound(11, qRound(box.height() * 0.38), 14);
          f.setPixelSize(targetPx);
        } else if (i > 0 && !hasFill) {
          f.setWeight(QFont::DemiBold);
          const int targetPx = compactMode
                                   ? qBound(10, qRound(box.height() * 0.85), 13)
                                   : qBound(12, qRound(box.height() * 0.56), 17);
          f.setPixelSize(targetPx);
        } else {
          f.setPixelSize(qBound(9, qRound(box.height() * 0.45), 14));
        }
        p.setFont(f);
        const qreal padX = qMin(10.0, qMax(0.0, (box.width() - 8.0) * 0.5));
        const qreal padY =
            compactMode ? 0.0 : qMin(2.0, qMax(0.0, (box.height() - 4.0) * 0.5));
        QRectF textBox = box.adjusted(padX, padY, -padX, -padY);
        if (!hasFill && textBox.height() < f.pixelSize() + 6) {
          const qreal expand = (f.pixelSize() + 6 - textBox.height()) * 0.5;
          textBox.adjust(0, -expand, 0, expand);
        }
        p.drawText(textBox, Qt::AlignVCenter | Qt::AlignLeft | Qt::TextWordWrap,
                   text);
      }

      if (!selectedRole.isEmpty() && e.value("enabled").toBool() &&
          str(e, "role") == selectedRole) {
        const double coverage =
            (a[2].toDouble() * a[3].toDouble()) / qMax(1.0, sceneW * sceneH);
        if (i == 0 || coverage >= 0.85)
          backgroundSelected = true;
        else
          selectedBoxes.append(box);
      }
    }

    // Subtle top sheen inside the scene frame for refined window depth
    const qreal sheenH = qMin(20.0, r.height() * 0.16);
    QLinearGradient topSheen(r.left(), r.top(), r.left(), r.top() + sheenH);
    topSheen.setColorAt(
        0.0, QColor(255, 255, 255, baseBg.lightness() < 128 ? 16 : 24));
    topSheen.setColorAt(1.0, QColor(255, 255, 255, 0));
    p.fillRect(QRectF(r.left(), r.top(), r.width(), sheenH), topSheen);
    p.setPen(QPen(QColor(255, 255, 255, baseBg.lightness() < 128 ? 28 : 45), 1.0));
    p.drawLine(QPointF(frame.left() + radius, frame.top() + 1.0),
               QPointF(frame.right() - radius, frame.top() + 1.0));

    const QColor hi = palette().color(QPalette::Active, QPalette::Highlight);
    if (backgroundSelected) {
      p.setPen(QPen(hi, 2.0));
      p.setBrush(Qt::NoBrush);
      p.drawRoundedRect(r.adjusted(1.5, 1.5, -1.5, -1.5),
                        qMax(1.5, radius - 1.0), qMax(1.5, radius - 1.0));
    }
    if (!selectedBoxes.isEmpty()) {
      const qreal selRadius = compactMode ? 3.0 : 4.0;
      p.setBrush(Qt::NoBrush);
      for (const auto &sb : selectedBoxes) {
        p.setPen(QPen(hi, 2.0));
        p.drawRoundedRect(sb.adjusted(1.0, 1.0, -1.0, -1.0), selRadius,
                          selRadius);
      }
    }
    if (hoveredIndex >= 0 && hoveredIndex < elements.size()) {
      const auto e = elements[hoveredIndex].toObject();
      const QRectF hb = elementWidgetRect(e, r);
      if (!hb.isEmpty()) {
        QColor hoverPen = hi;
        hoverPen.setAlpha(dragHover ? 230 : 150);
        p.setPen(QPen(hoverPen, dragHover ? 2.0 : 1.25));
        p.setBrush(Qt::NoBrush);
        p.drawRoundedRect(hb.adjusted(1.0, 1.0, -1.0, -1.0),
                          compactMode ? 3.0 : 4.0, compactMode ? 3.0 : 4.0);
      }
    }

    // Selected-role frosted pill badge in the bottom-right corner
    if (!selectedRole.isEmpty() && r.width() >= 150 && r.height() >= 48) {
      QString title = selectedRoleTitle;
      if (title.isEmpty()) {
        title = selectedRole;
        title.replace('_', ' ');
        if (!title.isEmpty())
          title[0] = title[0].toUpper();
      }
      const QString badgeText = QStringLiteral("Selected: %1").arg(title);
      QFont badgeFont = font();
      badgeFont.setStyleStrategy(QFont::PreferAntialias);
      badgeFont.setPointSizeF(compactMode ? 8.0 : 8.5);
      badgeFont.setWeight(QFont::DemiBold);
      const QFontMetricsF bfm(badgeFont);
      const qreal dotDia = 6.0;
      const qreal dotGap = 5.0;
      const qreal hPad = 8.0;
      const qreal vPad = compactMode ? 2.5 : 3.5;
      const qreal textW = bfm.horizontalAdvance(badgeText);
      const qreal pillW = hPad + dotDia + dotGap + textW + hPad;
      const qreal pillH = bfm.height() + vPad * 2.0;
      const qreal margin = compactMode ? 6.0 : 9.0;
      if (pillW <= r.width() - margin * 2.0) {
        const QRectF pillRect(r.right() - margin - pillW,
                              r.bottom() - margin - pillH, pillW, pillH);
        const bool darkScene = baseBg.lightness() < 128;
        const QColor pillBg = darkScene ? QColor(14, 19, 27, 204)
                                        : QColor(248, 250, 252, 224);
        const QColor pillFg = darkScene ? QColor(240, 245, 250, 242)
                                        : QColor(22, 27, 34, 242);
        QColor pillBorder = hi;
        pillBorder.setAlpha(darkScene ? 175 : 190);
        p.setBrush(pillBg);
        p.setPen(QPen(pillBorder, 1.0));
        p.drawRoundedRect(pillRect, pillH * 0.5, pillH * 0.5);

        const QPointF dotCenter(pillRect.left() + hPad + dotDia * 0.5,
                                pillRect.center().y());
        p.setPen(Qt::NoPen);
        p.setBrush(hi);
        p.drawEllipse(dotCenter, dotDia * 0.5, dotDia * 0.5);

        p.setFont(badgeFont);
        p.setPen(pillFg);
        const QRectF badgeTextRect(pillRect.left() + hPad + dotDia + dotGap,
                                   pillRect.top(), textW + 2.0, pillH);
        p.drawText(badgeTextRect, Qt::AlignVCenter | Qt::AlignLeft, badgeText);
      }
    }
    p.restore();

    const bool darkSurface = palette().color(QPalette::Window).lightness() < 128;
    QColor border =
        darkSurface ? QColor(255, 255, 255, 58) : QColor(0, 0, 0, 72);
    qreal borderWidth = 1.0;
    if (dragHover || hasFocus()) {
      border = hi;
      borderWidth = 1.5;
    }
    p.setPen(QPen(border, borderWidth));
    p.setBrush(Qt::NoBrush);
    p.drawRoundedRect(frame, radius, radius);
  }
  void mouseMoveEvent(QMouseEvent *e) override {
    const int idx = hitIndex(e->pos());
    bool interactive = false;
    if (idx >= 0) {
      const auto item = array(scene, "elements")[idx].toObject();
      interactive = !str(item, "role").isEmpty() || !str(item, "target").isEmpty();
    }
    setCursor(interactive ? Qt::PointingHandCursor : Qt::ArrowCursor);
    const int nextHover = interactive ? idx : -1;
    if (hoveredIndex != nextHover) {
      hoveredIndex = nextHover;
      update();
    }
    QWidget::mouseMoveEvent(e);
  }
  void leaveEvent(QEvent *e) override {
    setCursor(Qt::ArrowCursor);
    if (hoveredIndex != -1) {
      hoveredIndex = -1;
      update();
    }
    QWidget::leaveEvent(e);
  }
  void mousePressEvent(QMouseEvent *e) override {
    auto item = hit(e->pos());
    auto role = str(item, "role"), target = str(item, "target");
    if (!role.isEmpty())
      send({{"op", "theme_select"}, {"role", role}});
    if (!target.isEmpty())
      send({{"op", "theme_scene"}, {"id", target}});
  }
  void dragEnterEvent(QDragEnterEvent *e) override {
    if (e->mimeData()->hasText()) {
      dragHover = true;
      hoveredIndex = hitIndex(e->position().toPoint());
      update();
      e->acceptProposedAction();
    }
  }
  void dragMoveEvent(QDragMoveEvent *e) override {
    if (e->mimeData()->hasText()) {
      const int idx = hitIndex(e->position().toPoint());
      if (idx != hoveredIndex) {
        hoveredIndex = idx;
        update();
      }
      e->acceptProposedAction();
    }
  }
  void dragLeaveEvent(QDragLeaveEvent *) override {
    dragHover = false;
    hoveredIndex = -1;
    update();
  }
  void dropEvent(QDropEvent *e) override {
    dragHover = false;
    hoveredIndex = -1;
    update();
    auto item = hit(e->position().toPoint());
    auto role = str(item, "role"), c = e->mimeData()->text();
    if (!role.isEmpty() && color(c).isValid()) {
      send({{"op", "theme_assign"}, {"role", role}, {"color", c}});
      e->acceptProposedAction();
    }
  }
};
class ThemeRoleCard : public QWidget {
  bool selected = false;
  std::function<void()> onSelect;

public:
  ThemeRoleCard(bool isSelected, std::function<void()> selectCallback,
                QWidget *parent = nullptr)
      : QWidget(parent), selected(isSelected),
        onSelect(std::move(selectCallback)) {}

protected:
  void paintEvent(QPaintEvent *) override {
    if (!selected)
      return;
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing, true);
    const QColor hi = palette().color(QPalette::Active, QPalette::Highlight);
    QColor fill = hi;
    fill.setAlpha(22);
    QColor border = hi;
    border.setAlpha(110);
    p.setBrush(fill);
    p.setPen(QPen(border, 1.0));
    p.drawRoundedRect(QRectF(rect()).adjusted(0.5, 0.5, -0.5, -0.5), 4.0, 4.0);
  }
  void mousePressEvent(QMouseEvent *e) override {
    if (e->button() == Qt::LeftButton && onSelect)
      onSelect();
    QWidget::mousePressEvent(e);
  }
};
class Frontend : public QWidget {
  Callback callback;
  void *context;
  QString mode;
  QJsonObject state;
  QTimer timer;
  QSettings settings;
  QTabWidget *tabs = nullptr;
  QPointer<QToolBar> mainToolbar;
  QList<QPointer<QToolButton>> navButtons;
  QLabel *status = nullptr, *connectionStatus = nullptr;
  StatusDot *connectionDot = nullptr;
  QAction *pickAction = nullptr;
  QShortcut *undoShortcut = nullptr, *redoShortcut = nullptr;
  QVBoxLayout *root;
  QVBoxLayout *historyLayout = nullptr;
  QWidget *historyPanel = nullptr, *historyBox = nullptr;
  QPointer<QLabel> historyEmptyPrimary, historyEmptyHint;
  QScrollArea *historyScroll = nullptr;
  QSplitter *splitter = nullptr, *verticalSplitter = nullptr;
  QWidget *pinnedScroll = nullptr, *pinnedCurrent = nullptr;
  QVBoxLayout *boardContent = nullptr;
  QPushButton *configSave = nullptr, *configRebind = nullptr;
  QLabel *configStatus = nullptr, *configStatusIcon = nullptr,
         *shortcutStatus = nullptr, *limitError = nullptr;
  QWidget *limitErrorBox = nullptr, *configStatusBox = nullptr,
          *shortcutStatusBox = nullptr;
  StatusDot *shortcutDot = nullptr;
  HistoryLimitSpinBox *configLimit = nullptr;
  QScrollArea *configScroll = nullptr;
  QList<QPushButton *> historyButtons;
  bool hiddenForPick = false, spectrumDragging = false;
  bool renderingBoard = false, hasPendingBoardRender = false;
  int mixerFormat = 0;
  QPointer<Spectrum> mixerSpectrum;
  QPointer<MixerWheel> mixerWheel;
  QPointer<QLabel> mixerReadout;
  QWidget *themeContainer = nullptr, *themeBody = nullptr;
  QPointer<QDialog> themeProjectDialog;
  QPointer<QComboBox> themePluginsCombo, themeSavedCombo;
  QPointer<QLineEdit> themeNameEdit;
  QPointer<QPushButton> themeOpenBtn, themeNewBtn;
  QJsonObject configDraft, generationDraft, themeDraft;
  bool compactThemePreview = false;
  bool themeProjectVisible = false;
  bool configInitialized = false;
  bool limitInvalid = false;
  bool limitErrorDismissed = false;
  static void suppressChildSignals(QWidget *w) {
    if (!w)
      return;
    for (auto *e : w->findChildren<QLineEdit *>())
      e->blockSignals(true);
    for (auto *s : w->findChildren<QAbstractSpinBox *>())
      s->blockSignals(true);
    for (auto *c : w->findChildren<QComboBox *>())
      c->blockSignals(true);
  }
  void commitFocusedEditor(QWidget *scope) {
    auto *focused = QApplication::focusWidget();
    if (!focused || (scope && focused != scope && !scope->isAncestorOf(focused)))
      return;
    if (auto *edit = qobject_cast<QLineEdit *>(focused)) {
      const bool wasModified = edit->isModified();
      QPointer<QLineEdit> guarded(edit);
      edit->clearFocus();
      if (!wasModified && guarded)
        QMetaObject::invokeMethod(guarded, "editingFinished");
    } else if (auto *spin = qobject_cast<QAbstractSpinBox *>(focused)) {
      spin->interpretText();
      spin->clearFocus();
    }
  }
  QJsonValue request(QJsonObject req) {
    auto bytes = QJsonDocument(req).toJson(QJsonDocument::Compact);
    char *answer = callback(context, bytes.constData());
    if (!answer)
      return {};
    QByteArray data(answer);
    swatch_qt_free(answer);
    QJsonParseError err;
    auto doc = QJsonDocument::fromJson(data, &err);
    if (err.error != QJsonParseError::NoError) {
      if (data.trimmed() == "null")
        return {};
      return QJsonObject{
          {"error", QString("Invalid response: ") + err.errorString()}};
    }
    return doc.isObject() ? QJsonValue(doc.object()) : QJsonValue();
  }
  void send(QJsonObject req, bool forceUpdate = false) {
    auto result = request(req);
    if (result.isObject())
      updateState(result.toObject(), forceUpdate);
  }
  QString activeEditor() const {
    static const QStringList editors = {"mixer", "palette", "theme"};
    return editors.value(tabs->currentIndex());
  }
  void updateUndoShortcuts() {
    if (!undoShortcut || !tabs) return;
    const auto editor = object(state.value(activeEditor()));
    undoShortcut->setEnabled(editor.value("can_undo").toBool());
    redoShortcut->setEnabled(editor.value("can_redo").toBool());
  }
  void updateState(const QJsonObject &next, bool force = false) {
    if (!force && next == state)
      return;
    auto previous = state;
    if (mode == "config" &&
        (force ||
         str(next, "validation_field") != str(previous, "validation_field") ||
         str(next, "message") != str(previous, "message")))
      limitErrorDismissed = false;
    state = next;
    if (state.contains("clipboard") && state.value("clipboard").isString())
      QApplication::clipboard()->setText(str(state, "clipboard"));
    if (mode == "board") {
      bool picking = state.value("picking").toBool();
      if (hiddenForPick && !picking) {
        show();
        hiddenForPick = false;
      }
      if (state.value("activate").toBool()) {
        showNormal();
        raise();
        activateWindow();
      }
      renderBoard(previous);
      if (state.value("show_palette").toBool() && tabs)
        tabs->setCurrentIndex(1);
      updateUndoShortcuts();
    } else if (mode == "config")
      renderConfig();
    else
      renderHistory();
  }
  QString message() const {
    QString s = str(state, "error");
    if (s.isEmpty())
      s = str(state, "load_error");
    if (s.isEmpty())
      s = str(state, "message");
    return s;
  }
  QString statusSummary() const {
    const QString msg = message();
    if (!msg.isEmpty())
      return msg;
    const int count = array(object(state.value("palette")), "swatches").size();
    if (count == 0)
      return QStringLiteral("Palette empty");
    if (count == 1)
      return QStringLiteral("1 swatch in palette");
    return QStringLiteral("%1 swatches in palette").arg(count);
  }
  void updateNavButtonStyles() {
    for (const auto &btn : navButtons)
      if (btn)
        btn->update();
  }
  void updateHistoryEmptyState(int panelW) {
    if (historyEmptyPrimary)
      historyEmptyPrimary->setText(panelW < 105
                                       ? QStringLiteral("No history")
                                       : QStringLiteral("No history yet"));
    if (historyEmptyHint)
      historyEmptyHint->setVisible(panelW >= 115);
    if (historyBox)
      historyBox->updateGeometry();
  }
  void clearToolbarFocus() {
    auto *focused = QApplication::focusWidget();
    if (!focused || (mainToolbar && mainToolbar->isAncestorOf(focused))) {
      if (tabs)
        tabs->setFocus(Qt::OtherFocusReason);
      else if (focused)
        focused->clearFocus();
    }
  }
  static constexpr int compactHistoryWidth = 92;
  bool compactShelf = false;
  int normalPaletteShelfHeight = 76;
  bool compactHistory = false;
  int normalHistoryWidth = 124;
  void sizeHistorySidebar() {
    if (!splitter || !historyPanel || !historyPanel->isVisible())
      return;
    const bool narrow = width() < 680;
    if (!narrow && !compactHistory)
      normalHistoryWidth =
          qMax(compactHistoryWidth, splitter->sizes().value(0, 124));
    if (narrow != compactHistory) {
      const int target = narrow ? compactHistoryWidth : normalHistoryWidth;
      splitter->setSizes({target, qMax(1, splitter->width() - target)});
      compactHistory = narrow;
    }
  }
  int headerBarHeight() const {
    return qMax(34, fontMetrics().height() + 14);
  }
  bool shortMixer() const {
    return height() <= qMax(360, fontMetrics().height() * 24);
  }
  void sizePinnedPalette() {
    if (!pinnedScroll || !tabs) return;
    auto page = tabs->widget(2);
    bool compact = tabs->currentIndex() == 2 && page &&
                   page->height() < page->fontMetrics().height() * 22;
    const auto swatches = array(object(state.value("palette")), "swatches");
    const bool mixerEmptyShelf = tabs->currentIndex() == 0 && shortMixer() &&
                                 swatches.isEmpty();
    const int shelfThreshold = fontMetrics().height() * 24;
    const bool shrinkShelf = (tabs->currentIndex() == 1 && height() < shelfThreshold) ||
                             mixerEmptyShelf;
    const int dropHeight = fontMetrics().height() + 8;
    int hbarExtent = 0;
    if (auto *sa = pinnedScroll->findChild<QScrollArea *>("pinnedShelfScroll")) {
      sa->setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
      if (!swatches.isEmpty()) {
        const int contentMinW =
            swatches.size() * 96 + qMax(0, swatches.size() - 1) * 4 + 12;
        const int availW = sa->viewport()->width() > 0
                               ? sa->viewport()->width()
                               : qMax(1, width() - 2);
        if (contentMinW > availW)
          hbarExtent =
              sa->style()->pixelMetric(QStyle::PM_ScrollBarExtent, nullptr, sa);
      }
    }
    const int minShelfH = mixerEmptyShelf ? dropHeight : (42 + hbarExtent);
    const int compactThemeH =
        page ? qMax(page->fontMetrics().height() * 3, minShelfH) : minShelfH;
    if (verticalSplitter && shrinkShelf && !compactShelf)
      normalPaletteShelfHeight = verticalSplitter->sizes().value(1, 76);
    pinnedScroll->setMinimumHeight(minShelfH);
    pinnedScroll->setMaximumHeight(mixerEmptyShelf ? dropHeight :
                                   compact ? compactThemeH : QWIDGETSIZE_MAX);
    if (verticalSplitter && shrinkShelf != compactShelf) {
      verticalSplitter->setSizes({verticalSplitter->height(),
                                  shrinkShelf ? minShelfH
                                              : qMax(normalPaletteShelfHeight, minShelfH)});
      compactShelf = shrinkShelf;
    } else if (verticalSplitter && compactShelf &&
               verticalSplitter->sizes().value(1, 0) < minShelfH) {
      verticalSplitter->setSizes(
          {qMax(1, verticalSplitter->height() - minShelfH), minShelfH});
    }
    if (compact && verticalSplitter)
      verticalSplitter->setSizes({verticalSplitter->height(), compactThemeH});
  }
  void preserveView(QWidget *old, QWidget *replacement) {
    auto oldScroll = qobject_cast<QScrollArea *>(old),
         newScroll = qobject_cast<QScrollArea *>(replacement);
    if (!oldScroll && (old->objectName() == "themePage" || old->objectName() == "themeBody" ||
                       old->objectName() == "mixerPage" ||
                       old->objectName() == "palettePage" || old->objectName() == "pinnedShelf"))
      oldScroll = old->findChild<QScrollArea *>();
    if (!newScroll && (replacement->objectName() == "themePage" || replacement->objectName() == "themeBody" ||
                       replacement->objectName() == "mixerPage" ||
                       replacement->objectName() == "palettePage" || replacement->objectName() == "pinnedShelf"))
      newScroll = replacement->findChild<QScrollArea *>();
    int vertical = oldScroll ? oldScroll->verticalScrollBar()->value() : 0,
        horizontal = oldScroll ? oldScroll->horizontalScrollBar()->value() : 0;
    QWidget *focus = QApplication::focusWidget();
    QString name, text;
    int occurrence = 0, cursor = 0;
    bool modified = false;
    if (focus && (focus == old || old->isAncestorOf(focus))) {
      name = focus->accessibleName();
      auto list = old->findChildren<QWidget *>();
      for (auto item : list) {
        if (item == focus)
          break;
        if (item->accessibleName() == name &&
            item->metaObject() == focus->metaObject())
          ++occurrence;
      }
      if (auto edit = qobject_cast<QLineEdit *>(focus)) {
        text = edit->text();
        cursor = edit->cursorPosition();
        modified = edit->isModified();
      }
    }
    QWidget *target = nullptr;
    if (focus && !name.isEmpty())
      for (auto item : replacement->findChildren<QWidget *>())
        if (item->accessibleName() == name &&
            item->metaObject() == focus->metaObject()) {
          if (occurrence-- == 0) {
            target = item;
            break;
          }
        }
    if (auto edit = qobject_cast<QLineEdit *>(target); edit && modified) {
      QSignalBlocker block(edit);
      edit->setText(text);
      edit->setModified(true);
      edit->setCursorPosition(cursor);
    }
    QPointer<QWidget> guarded(target);
    QPointer<QScrollArea> scroll(newScroll);
    QTimer::singleShot(0, this, [guarded, scroll, vertical, horizontal] {
      if (scroll) {
        scroll->verticalScrollBar()->setValue(vertical);
        scroll->horizontalScrollBar()->setValue(horizontal);
      }
      if (guarded)
        guarded->setFocus(Qt::OtherFocusReason);
    });
  }
  void replace(QWidget *w) {
    while (auto item = root->takeAt(0)) {
      if (item->widget()) {
        preserveView(item->widget(), w);
        suppressChildSignals(item->widget());
        item->widget()->hide();
        item->widget()->setParent(nullptr);
        item->widget()->deleteLater();
      }
      delete item;
    }
    root->addWidget(w);
    if (isVisible()) {
      w->show();
      root->activate();
    }
  }
  QComboBox *combo(const QString &name, QJsonArray values, QWidget *parent,
                   const QString &selected = {}) {
    auto c = new QComboBox(parent);
    c->setAccessibleName(name);
    for (auto v : values) {
      auto o = v.toObject();
      c->addItem(str(o, "name").isEmpty() ? str(o, "label") : str(o, "name"),
                 str(o, "id"));
    }
    int i = c->findData(selected);
    if (i >= 0)
      c->setCurrentIndex(i);
    return c;
  }
  QString actionHint(const QString &label, const QString &iconName) const {
    if (!iconName.startsWith("edit-undo") && !iconName.startsWith("edit-redo"))
      return label;
    auto key = iconName.startsWith("edit-undo") ? QKeySequence::Undo
                                                : QKeySequence::Redo;
    return label + " (" + QKeySequence(key).toString(QKeySequence::NativeText) + ")";
  }
  QToolButton *toolButton(const QString &label, const QStringList &iconNames,
                          QWidget *parent, const std::function<void()> &action,
                          QStyle::StandardPixmap fallback = QStyle::SP_FileIcon) {
    auto b = new QToolButton(parent);
    b->setText(label);
    b->setAccessibleName(label);
    b->setToolTip(actionHint(label, iconNames.value(0)));
    b->setProperty("boardAction", true);
    b->setIcon(themedIcon(iconNames, fallback));
    b->setToolButtonStyle(settings.value("board/toolbarLabels", true).toBool()
                              ? Qt::ToolButtonTextBesideIcon : Qt::ToolButtonIconOnly);
    connect(b, &QToolButton::clicked, this, action);
    return b;
  }
  QToolButton *toolButton(const QString &label, const QString &iconName,
                          QWidget *parent, const std::function<void()> &action,
                          QStyle::StandardPixmap fallback = QStyle::SP_FileIcon) {
    return toolButton(label, QStringList{iconName}, parent, action, fallback);
  }
  QStyle::StandardPixmap actionFallback(const QString &op) const {
    if (op == "mixer_copy" || op.contains("copy"))
      return QStyle::SP_FileLinkIcon;
    if (op == "palette_add_current")
      return QStyle::SP_DialogYesButton;
    if (op == "mixer_source_add")
      return QStyle::SP_FileDialogNewFolder;
    if (op.contains("undo"))
      return QStyle::SP_ArrowBack;
    if (op.contains("redo"))
      return QStyle::SP_ArrowForward;
    if (op == "mixer_mix" || op == "theme_palette_bind")
      return QStyle::SP_DialogApplyButton;
    if (op.contains("duplicate"))
      return QStyle::SP_FileLinkIcon;
    if (op.contains("save"))
      return QStyle::SP_DialogSaveButton;
    if (op.contains("new"))
      return QStyle::SP_FileIcon;
    return QStyle::SP_DialogOpenButton;
  }
  QPixmap makeEmptyHistoryIcon(int iconSz, int alpha) const {
    const qreal dpr = devicePixelRatioF();
    const QStringList names{"view-history-symbolic", "view-history",
                            "document-open-recent"};
    for (const auto &name : names) {
      if (QIcon::hasThemeIcon(name)) {
        QIcon icon = QIcon::fromTheme(name);
        if (!icon.isNull()) {
          QPixmap raw = icon.pixmap(QSize(iconSz, iconSz), dpr);
          QPixmap tinted(raw.size());
          tinted.setDevicePixelRatio(raw.devicePixelRatio());
          tinted.fill(Qt::transparent);
          QPainter tp(&tinted);
          tp.drawPixmap(0, 0, raw);
          tp.setCompositionMode(QPainter::CompositionMode_SourceIn);
          tp.fillRect(tinted.rect(), mutedColor(palette(), alpha));
          tp.end();
          return tinted;
        }
      }
    }
    const int px = qMax(1, qRound(iconSz * dpr));
    QPixmap pm(px, px);
    pm.setDevicePixelRatio(dpr);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing, true);
    const QColor ink = mutedColor(palette(), alpha);
    p.setPen(QPen(ink, 1.6, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    p.setBrush(Qt::NoBrush);
    const QRectF ring(2.5, 2.5, iconSz - 5.0, iconSz - 5.0);
    p.drawArc(ring, 150 * 16, -310 * 16);
    const QPointF center(iconSz / 2.0, iconSz / 2.0);
    p.drawLine(center, center + QPointF(0.0, -iconSz * 0.22));
    p.drawLine(center, center + QPointF(iconSz * 0.17, iconSz * 0.1));
    const QPointF tip(ring.left() + 0.6, ring.top() + ring.height() * 0.25);
    p.drawLine(tip, tip + QPointF(0.0, -4.0));
    p.drawLine(tip, tip + QPointF(4.0, 0.0));
    p.end();
    return pm;
  }
  QStringList actionIcon(const QString &op) const {
    if (op == "mixer_copy" || op.contains("copy"))
      return {"edit-copy-symbolic", "edit-copy"};
    if (op == "palette_add_current" || op == "mixer_source_add")
      return {"list-add-symbolic", "list-add"};
    if (op.contains("undo"))
      return {"edit-undo-symbolic", "edit-undo"};
    if (op.contains("redo"))
      return {"edit-redo-symbolic", "edit-redo"};
    if (op == "mixer_mix")
      return {"merge-symbolic", "merge", "color-mode-black-white-symbolic",
              "color-mode-black-white"};
    if (op == "theme_palette_bind")
      return {"color-fill-symbolic", "color-fill"};
    if (op.contains("duplicate"))
      return {"edit-duplicate-symbolic", "edit-duplicate"};
    if (op.contains("save"))
      return {"document-save-symbolic", "document-save"};
    if (op.contains("new"))
      return {"document-new-symbolic", "document-new"};
    return {"list-add-symbolic", "list-add"};
  }
  QIcon themedIcon(const QStringList &names,
                   QStyle::StandardPixmap fallback) const {
    for (const auto &name : names) {
      auto icon = QIcon::fromTheme(name);
      if (!icon.isNull())
        return icon;
    }
    return style()->standardIcon(fallback);
  }
  QWidget *scroll(QWidget *content) {
    auto s = new QScrollArea;
    s->setFrameShape(QFrame::NoFrame);
    s->setWidgetResizable(true);
    s->setWidget(content);
    if (content->objectName() == "mixerContent")
      s->viewport()->installEventFilter(this);
    return s;
  }
  QWidget *pinnedShelf() {
    auto container = new QWidget;
    container->setObjectName("pinnedShelf");
    container->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
    container->setMinimumHeight(42);
    auto l = new QVBoxLayout(container);
    l->setContentsMargins(0, 0, 0, 0);
    l->setSpacing(0);
    l->addWidget(new Hairline(container));
    auto s = static_cast<QScrollArea *>(scroll(pinnedPalette()));
    s->setObjectName("pinnedShelfScroll");
    s->setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    s->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
    l->addWidget(s, 1);
    return container;
  }
  QWidget *pinnedPalette() {
    auto p = object(state.value("palette"));
    auto w = new QWidget;
    auto l = new QVBoxLayout(w);
    l->setSpacing(0);
    auto swatches = array(p, "swatches");
    if (swatches.isEmpty()) {
      l->setContentsMargins(0, 0, 0, 0);
      l->addWidget(new PaletteDrop(
          [this](int i, QString c) {
            if (i >= 0)
              send({{"op", "history_select"}, {"index", i}, {"tab", 1}});
            else {
              send({{"op", "mixer_select"}, {"color", c}});
              send({{"op", "palette_add_current"}});
            }
          }, w));
      return w;
    }
    l->setContentsMargins(6, 6, 6, 6);
    auto swatchRow = new QHBoxLayout;
    swatchRow->setSpacing(4);
    l->addLayout(swatchRow);
    int i = 0;
    for (auto value : swatches) {
      auto o = value.toObject();
      int index = i++;
      auto c = color(str(o, "color"));
      const bool locked = o.value("locked").toBool();
      auto sw = new Swatch(c, str(o, "name"), w);
      sw->setText(str(o, "color").toUpper());
      sw->setMinimumSize(96, 24);
      sw->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
      sw->setTileMode(true, locked);
      sw->setToolTip(
          str(o, "name") + (locked ? " (locked)" : "") +
          " — click to select, drag to reorder; right-click for options");
      connect(sw, &QPushButton::clicked, this, [this, c] {
        send({{"op", "mixer_select"}, {"color", hex(c)}});
        tabs->setCurrentIndex(0);
      });
      sw->paletteDrop(index, [this, index, o](int from, QString value) {
        if (from >= 0 && from != index)
          send({{"op", "palette_move"}, {"from", from}, {"to", index}});
        else if (from < 0 && color(value).isValid())
          send({{"op", "palette_edit"}, {"index", index},
                {"name", str(o, "name")}, {"color", value}});
      });
      sw->setContextMenuPolicy(Qt::CustomContextMenu);
      connect(sw, &QWidget::customContextMenuRequested, this,
              [this, sw, index, o](const QPoint &point) {
        QMenu menu;
        auto edit = menu.addAction("Edit name and color…");
        auto lock = menu.addAction(o.value("locked").toBool() ? "Unlock" : "Lock");
        auto remove = menu.addAction("Delete");
        auto chosen = menu.exec(sw->mapToGlobal(point));
        if (chosen == edit) {
          bool ok = false;
          auto name = QInputDialog::getText(this, "Edit swatch", "Name", QLineEdit::Normal, str(o, "name"), &ok);
          if (!ok) return;
          auto value = QInputDialog::getText(this, "Edit swatch", "Color", QLineEdit::Normal, str(o, "color"), &ok);
          if (ok) send({{"op", "palette_edit"}, {"index", index}, {"name", name}, {"color", value}});
        } else if (chosen == lock) {
          send({{"op", "palette_lock"}, {"index", index}, {"locked", !o.value("locked").toBool()}});
        } else if (chosen == remove) {
          send({{"op", "palette_delete_swatch"}, {"index", index}});
        }
      });
      swatchRow->addWidget(sw, 1);
    }
    return w;
  }
  void renderBoard(const QJsonObject &previous) {
    if (renderingBoard) {
      hasPendingBoardRender = true;
      return;
    }
    renderingBoard = true;
    QJsonObject prev = previous;
    for (;;) {
      hasPendingBoardRender = false;
      const QJsonObject currentSnapshot = state;
      bool first = !tabs;
      if (first) {
      auto outer = new QWidget;
      auto layout = new QVBoxLayout(outer);
      layout->setContentsMargins(0, 0, 0, 0);
      layout->setSpacing(0);
      auto toolbar = new QToolBar(outer);
      mainToolbar = toolbar;
      toolbar->setMovable(false);
      toolbar->setFloatable(false);
      toolbar->setFocusPolicy(Qt::NoFocus);
      toolbar->setContentsMargins(4, 2, 4, 2);
      toolbar->setIconSize(QSize(16, 16));
      if (auto *tl = toolbar->layout())
        tl->setSpacing(4);
      bool labels = settings.value("board/toolbarLabels", true).toBool();
      toolbar->setToolButtonStyle(labels ? Qt::ToolButtonTextBesideIcon : Qt::ToolButtonIconOnly);
      auto addAction = [this, toolbar](const QString &name,
                                       const QStringList &icons,
                                       QStyle::StandardPixmap fallback,
                                       const std::function<void()> &action) {
        auto a = toolbar->addAction(themedIcon(icons, fallback), name);
        connect(a, &QAction::triggered, this, action);
        return a;
      };
      auto nav = new QActionGroup(toolbar);
      nav->setExclusive(true);
      navButtons.clear();
      QList<QAction *> pages;
      struct NavEntry {
        QString label;
        QStringList icons;
        QStyle::StandardPixmap fallback;
        Qt::Key key;
      };
      for (const auto &entry : QList<NavEntry>{
               {"Mixer",
                {"color-mode-black-white-symbolic", "color-mode-black-white",
                 "dialog-fill-and-stroke-symbolic", "color-gradient"},
                QStyle::SP_ComputerIcon,
                Qt::Key_1},
               {"Palette",
                {"color-fill-symbolic", "color-fill"},
                QStyle::SP_DirIcon,
                Qt::Key_2},
               {"Theme",
                {"preferences-desktop-theme-global-symbolic",
                 "preferences-desktop-theme-global",
                 "preferences-desktop-theme"},
                QStyle::SP_DesktopIcon,
                Qt::Key_3}}) {
        int index = pages.size();
        const QKeySequence shortcutSeq(Qt::CTRL | entry.key);
        auto *action = new QAction(themedIcon(entry.icons, entry.fallback),
                                   entry.label, nav);
        action->setCheckable(true);
        action->setToolTip(entry.label + " (" +
                           shortcutSeq.toString(QKeySequence::NativeText) + ")");
        nav->addAction(action);
        connect(action, &QAction::triggered, this, [this, index] {
          tabs->setCurrentIndex(index);
          clearToolbarFocus();
        });
        auto *navShortcut = new QShortcut(shortcutSeq, this);
        connect(navShortcut, &QShortcut::activated, this, [this, index] {
          if (tabs)
            tabs->setCurrentIndex(index);
        });
        auto *btn = new NavToolButton(toolbar);
        btn->setDefaultAction(action);
        btn->setIconSize(toolbar->iconSize());
        btn->setToolButtonStyle(toolbar->toolButtonStyle());
        connect(toolbar, &QToolBar::iconSizeChanged, btn,
                &QToolButton::setIconSize);
        connect(toolbar, &QToolBar::toolButtonStyleChanged, btn,
                &QToolButton::setToolButtonStyle);
        toolbar->addWidget(btn);
        navButtons.append(btn);
        pages.append(action);
      }
      updateNavButtonStyles();
      toolbar->addSeparator();
      auto pick = addAction("Pick color",
                            {"color-picker-symbolic", "color-picker"},
                            QStyle::SP_DialogOpenButton, [this] {
        if (state.value("hide_board_while_picking").toBool()) {
          hiddenForPick = true;
          hide();
          QGuiApplication::sync();
        }
        send({{"op", "pick"}});
      });
      pick->setToolTip("Pick a color from the screen");
      pickAction = pick;
      auto spacer = new QWidget(toolbar);
      spacer->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
      spacer->setMinimumWidth(0);
      spacer->setAttribute(Qt::WA_TransparentForMouseEvents);
      toolbar->addWidget(spacer);
      auto toggle = toolbar->addAction(
          themedIcon({"view-history-symbolic", "view-history",
                      "document-open-recent"},
                     QStyle::SP_FileDialogListView),
          "History");
      toggle->setToolTip("Show or hide the picked color history sidebar");
      toggle->setCheckable(true);
      toggle->setChecked(settings.value("board/historyVisible", true).toBool());
      auto labelToggle = toolbar->addAction(
          themedIcon({"view-list-text-symbolic", "view-list-text"},
                     QStyle::SP_FileDialogContentsView),
          "Show labels");
      labelToggle->setToolTip("Show or hide toolbar button labels");
      labelToggle->setCheckable(true);
      labelToggle->setChecked(labels);
      connect(labelToggle, &QAction::toggled, this, [this, toolbar](bool visible) {
        const auto buttonStyle = visible ? Qt::ToolButtonTextBesideIcon : Qt::ToolButtonIconOnly;
        toolbar->setToolButtonStyle(buttonStyle);
        settings.setValue("board/toolbarLabels", visible);
        for (auto bar : findChildren<QToolBar *>())
          if (bar->property("boardAction").toBool())
            bar->setToolButtonStyle(buttonStyle);
        for (auto b : findChildren<QToolButton *>())
          if (b->property("boardAction").toBool())
            b->setToolButtonStyle(buttonStyle);
        if (tabs && tabs->widget(0)) {
          if (auto *scroll = tabs->widget(0)->findChild<QScrollArea *>())
            layoutMixerContent(scroll->widget(), scroll->viewport()->size());
        }
      });
      toolbar->addSeparator();
      auto settingsAction = addAction(
          "Settings", {"configure-symbolic", "configure"},
          QStyle::SP_FileDialogDetailedView,
          [this] { send({{"op", "configure"}}); });
      settingsAction->setToolTip("Open Swatch Board settings");
      for (auto *btn : toolbar->findChildren<QToolButton *>())
        btn->setFocusPolicy(Qt::NoFocus);
      layout->addWidget(toolbar);
      layout->addWidget(new Hairline(outer));
      splitter = new QSplitter(Qt::Horizontal, outer);
      historyPanel = new QWidget(splitter);
      historyPanel->setAutoFillBackground(true);
      historyPanel->setBackgroundRole(QPalette::Window);
      historyPanel->setMinimumWidth(78);
      historyPanel->installEventFilter(this);
      auto historyOuter = new QHBoxLayout(historyPanel);
      historyOuter->setContentsMargins(0, 0, 0, 0);
      historyOuter->setSpacing(0);
      auto historyContent = new QWidget(historyPanel);
      historyContent->setBackgroundRole(QPalette::Window);
      auto historyPanelLayout = new QVBoxLayout(historyContent);
      historyPanelLayout->setContentsMargins(0, 0, 0, 0);
      historyPanelLayout->setSpacing(0);
      auto historyHeader = new QLabel("History", historyContent);
      historyHeader->setObjectName("historyHeader");
      QFont headerFont = historyHeader->font();
      headerFont.setWeight(QFont::DemiBold);
      historyHeader->setFont(headerFont);
      historyHeader->setAlignment(Qt::AlignLeft | Qt::AlignVCenter);
      historyHeader->setContentsMargins(10, 0, 10, 0);
      historyHeader->setFixedHeight(headerBarHeight());
      historyPanelLayout->addWidget(historyHeader);
      historyPanelLayout->addWidget(new Hairline(historyContent));
      historyBox = new QWidget;
      historyBox->setBackgroundRole(QPalette::Window);
      historyLayout = new QVBoxLayout(historyBox);
      historyLayout->setContentsMargins(6, 6, 6, 6);
      historyLayout->setSpacing(4);
      historyScroll = new QScrollArea(historyContent);
      historyScroll->setFrameShape(QFrame::NoFrame);
      historyScroll->setWidgetResizable(true);
      historyScroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
      historyScroll->setBackgroundRole(QPalette::Window);
      historyScroll->viewport()->setBackgroundRole(QPalette::Window);
      historyScroll->setWidget(historyBox);
      historyPanelLayout->addWidget(historyScroll, 1);
      historyOuter->addWidget(historyContent, 1);
      historyOuter->addWidget(new Hairline(Qt::Vertical, historyPanel));
      historyPanel->setVisible(toggle->isChecked());
      connect(toggle, &QAction::toggled, this, [this](bool visible) {
        historyPanel->setVisible(visible);
        settings.setValue("board/historyVisible", visible);
      });
      splitter->addWidget(historyPanel);
      tabs = new QTabWidget(outer);
      tabs->setDocumentMode(true);
      tabs->setAccessibleName("Board sections");
      tabs->setFocusProxy(nullptr);
      tabs->setFocusPolicy(Qt::ClickFocus);
      undoShortcut = new QShortcut(this);
      undoShortcut->setKeys(QKeySequence::keyBindings(QKeySequence::Undo));
      redoShortcut = new QShortcut(this);
      redoShortcut->setKeys(QKeySequence::keyBindings(QKeySequence::Redo));
      connect(undoShortcut, &QShortcut::activated, this, [this] {
        send({{"op", activeEditor() + "_undo"}});
      });
      connect(redoShortcut, &QShortcut::activated, this, [this] {
        send({{"op", activeEditor() + "_redo"}});
      });
      tabs->addTab(mixerPage(), "Mixer");
      tabs->addTab(palettePage(), "Palette");
      tabs->addTab(createThemePage(), "Theme");
      tabs->setCurrentIndex(
          qBound(0, settings.value("board/tab", 0).toInt(), 2));
      tabs->tabBar()->hide();
      auto content = new QWidget(splitter);
      boardContent = new QVBoxLayout(content);
      boardContent->setContentsMargins(0, 0, 0, 0);
      boardContent->setSpacing(0);
      pinnedCurrent = currentColor();
      boardContent->addWidget(pinnedCurrent);
      boardContent->addWidget(tabs, 1);
      pinnedCurrent->setVisible(tabs->currentIndex() == 0);
      pages[tabs->currentIndex()]->setChecked(true);
      connect(tabs, &QTabWidget::currentChanged, this,
              [this, pages](int i) {
                settings.setValue("board/tab", i);
                updateUndoShortcuts();
                pinnedCurrent->setVisible(i == 0);
                sizePinnedPalette();
                pages[i]->setChecked(true);
                updateNavButtonStyles();
                clearToolbarFocus();
              });
      splitter->addWidget(content);
      verticalSplitter = new QSplitter(Qt::Vertical, outer);
      verticalSplitter->addWidget(splitter);
      pinnedScroll = pinnedShelf();
      verticalSplitter->addWidget(pinnedScroll);
      splitter->setStretchFactor(1, 1);
      verticalSplitter->setStretchFactor(0, 1);
      if (!splitter->restoreState(
              settings.value("board/horizontalSplitter").toByteArray()))
        splitter->setSizes({124, 776});
      if (!verticalSplitter->restoreState(
              settings.value("board/verticalSplitter").toByteArray()))
        verticalSplitter->setSizes({574, 76});
      layout->addWidget(verticalSplitter, 1);
      layout->addWidget(new Hairline(outer));
      auto statusBar = new QWidget(outer);
      auto statusRow = new QHBoxLayout(statusBar);
      statusRow->setContentsMargins(10, 5, 10, 5);
      statusRow->setSpacing(6);
      QFont statusFont = font();
      if (statusFont.pointSizeF() > 0)
        statusFont.setPointSizeF(qMax(8.5, statusFont.pointSizeF() - 1.0));
      else if (statusFont.pixelSize() > 0)
        statusFont.setPixelSize(qMax(10, statusFont.pixelSize() - 1));
      QPalette statusPalette = palette();
      const QColor statusFg = mutedColor(statusPalette, 195);
      statusPalette.setColor(QPalette::WindowText, statusFg);
      statusPalette.setColor(QPalette::Text, statusFg);
      status = new QLabel(statusBar);
      status->setWordWrap(true);
      status->setFont(statusFont);
      status->setPalette(statusPalette);
      status->setAccessibleName("Board message");
      connectionDot = new StatusDot(statusBar);
      connectionStatus = new QLabel(statusBar);
      connectionStatus->setFont(statusFont);
      connectionStatus->setPalette(statusPalette);
      connectionStatus->setAccessibleName("Connection status");
      statusRow->addWidget(status, 1);
      statusRow->addWidget(connectionDot, 0, Qt::AlignVCenter);
      statusRow->addWidget(connectionStatus, 0, Qt::AlignVCenter);
      layout->addWidget(statusBar);
      replace(outer);
      clearToolbarFocus();
      QTimer::singleShot(0, this, [this] { clearToolbarFocus(); });
    } else {
      int current = tabs->currentIndex();
      if (prev.value("mixer") != state.value("mixer")) {
        auto replacement = currentColor();
        boardContent->replaceWidget(pinnedCurrent, replacement);
        pinnedCurrent->hide();
        pinnedCurrent->deleteLater();
        pinnedCurrent = replacement;
        pinnedCurrent->setVisible(current == 0);
      }
      QSignalBlocker blocker(tabs);
      auto refresh = [this](int index, QWidget *page, const QString &title) {
        auto old = tabs->widget(index);
        if (old) {
          preserveView(old, page);
          suppressChildSignals(old);
          tabs->removeTab(index);
          tabs->insertTab(index, page, title);
          old->hide();
          old->deleteLater();
        } else {
          tabs->insertTab(index, page, title);
        }
      };
      if (prev.value("mixer") != state.value("mixer")) {
        if (spectrumDragging) {
          auto m = object(state.value("mixer"));
          auto hsv = array(m, "hsv");
          auto rgba = array(m, "rgba");
          if (hsv.size() == 3) {
            if (mixerSpectrum) mixerSpectrum->setHsv(hsv[0].toDouble(), hsv[1].toDouble(), hsv[2].toDouble());
            if (mixerWheel) mixerWheel->setHsv(hsv[0].toDouble(), hsv[1].toDouble(), hsv[2].toDouble());
            if (mixerReadout)
              mixerReadout->setText(mixerHsvReadout(
                  hsv[0].toDouble(), hsv[1].toDouble(), hsv[2].toDouble(),
                  rgba.size() == 4 ? rgba[3].toInt() : 255));
          }
        } else
          refresh(0, mixerPage(), "Mixer");
      }
      if (prev.value("palette") != state.value("palette") ||
          (generationDraft.value("base").toString().isEmpty() &&
           prev.value("mixer") != state.value("mixer")))
        refresh(1, palettePage(), "Palette");
      if (prev.value("theme") != state.value("theme")) {
        Scene *reused = nullptr;
        if (themeBody &&
            object(prev.value("theme")).value("scene") ==
                object(state.value("theme")).value("scene")) {
          for (auto widget : themeBody->findChildren<QWidget *>())
            if ((reused = dynamic_cast<Scene *>(widget)))
              break;
        }
        refreshThemeContent(reused);
        syncThemeProjectDialog();
        if (object(prev.value("theme")).value("selected") !=
            object(state.value("theme")).value("selected")) {
          QPointer<QWidget> guarded(themeBody);
          QTimer::singleShot(0, this, [guarded] {
            if (!guarded) return;
            auto inspector = guarded->findChild<QScrollArea *>("themeInspector");
            auto role = guarded->findChild<QWidget *>("selectedThemeRole");
            if (inspector && role)
              inspector->ensureWidgetVisible(role, 0, 0);
          });
        }
      }
      if (prev.value("palette") != state.value("palette")) {
        auto replacement = pinnedShelf();
        preserveView(pinnedScroll, replacement);
        auto old = verticalSplitter->replaceWidget(1, replacement);
        pinnedScroll = replacement;
        sizePinnedPalette();
        if (old) {
          suppressChildSignals(old);
          old->hide();
          old->deleteLater();
        }
      }
      tabs->setCurrentIndex(current);
      pinnedCurrent->setVisible(current == 0);
      clearToolbarFocus();
    }
    if (first || prev.value("history") != state.value("history")) {
      historyEmptyPrimary = nullptr;
      historyEmptyHint = nullptr;
      while (auto item = historyLayout->takeAt(0)) {
        if (item->widget())
          item->widget()->deleteLater();
        delete item;
      }
      auto entries = array(state, "history");
      if (entries.isEmpty()) {
        historyLayout->setContentsMargins(8, 12, 8, 12);
        historyLayout->setSpacing(6);
        historyLayout->addStretch(1);
        QPixmap emptyIcon = makeEmptyHistoryIcon(22, 110);
        if (!emptyIcon.isNull()) {
          auto iconLabel = new QLabel(historyBox);
          iconLabel->setPixmap(emptyIcon);
          iconLabel->setAlignment(Qt::AlignCenter);
          historyLayout->addWidget(iconLabel, 0, Qt::AlignHCenter);
        }
        auto primary = new QLabel("No history yet", historyBox);
        primary->setWordWrap(true);
        primary->setAlignment(Qt::AlignHCenter | Qt::AlignTop);
        primary->setMinimumWidth(0);
        primary->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Preferred);
        QPalette primaryPal = primary->palette();
        const QColor primaryMuted = mutedColor(primaryPal, 145);
        primaryPal.setColor(QPalette::WindowText, primaryMuted);
        primaryPal.setColor(QPalette::Text, primaryMuted);
        primary->setPalette(primaryPal);
        historyEmptyPrimary = primary;
        historyLayout->addWidget(primary);
        auto hint = new QLabel("Use Pick color to capture", historyBox);
        hint->setWordWrap(true);
        hint->setAlignment(Qt::AlignHCenter | Qt::AlignTop);
        hint->setMinimumWidth(0);
        hint->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Preferred);
        QFont hintFont = hint->font();
        if (hintFont.pointSizeF() > 0)
          hintFont.setPointSizeF(qMax(8.0, hintFont.pointSizeF() - 1.0));
        else if (hintFont.pixelSize() > 0)
          hintFont.setPixelSize(qMax(9, hintFont.pixelSize() - 1));
        hint->setFont(hintFont);
        QPalette hintPal = hint->palette();
        const QColor hintMuted = mutedColor(hintPal, 118);
        hintPal.setColor(QPalette::WindowText, hintMuted);
        hintPal.setColor(QPalette::Text, hintMuted);
        hint->setPalette(hintPal);
        historyEmptyHint = hint;
        historyLayout->addWidget(hint);
        historyLayout->addStretch(1);
        const int panelW = historyPanel->width() > 0
                               ? historyPanel->width()
                               : (compactHistory ? compactHistoryWidth
                                                 : normalHistoryWidth);
        updateHistoryEmptyState(panelW);
      } else {
        historyLayout->setContentsMargins(6, 6, 6, 6);
        historyLayout->setSpacing(4);
        int index = 0;
        for (auto v : entries) {
          auto o = v.toObject();
          const QString entryText = str(o, "text");
          const QString detail = str(o, "detail");
          auto b = new Swatch(color(str(o, "color")), entryText, historyBox);
          b->setFlat(true);
          b->setHistoryRowMode(true);
          b->setToolTip(detail.isEmpty() ? entryText : detail);
          int i = index++;
          b->historyDrag(i);
          connect(b, &QPushButton::clicked, this, [this, i] {
            int tab = tabs->currentIndex();
            send({{"op", "history_select"}, {"index", i}, {"tab", tab}});
            if (tab != 1)
              tabs->setCurrentIndex(0);
          });
          historyLayout->addWidget(b);
        }
        historyLayout->addStretch();
      }
    }
    status->setText(statusSummary());
    const bool picking = state.value("picking").toBool();
    const bool loaded = state.value("loaded").toBool();
    const bool connected = state.value("connected").toBool();
    pickAction->setEnabled(loaded && connected && !picking);
    const QString connText = picking    ? QStringLiteral("Picking color…")
                             : !loaded  ? QStringLiteral("Loading…")
                             : connected ? QStringLiteral("Tray connected")
                                         : QStringLiteral("Tray disconnected");
    const QString connTip =
        picking
            ? QStringLiteral(
                  "Picking a color from the screen; press Escape or right-click to cancel")
        : !loaded
            ? QStringLiteral("Loading saved history and settings")
        : connected
            ? QStringLiteral("Connected to Swatch Board Tray for screen color picking")
            : QStringLiteral(
                  "Start Swatch Board Tray to enable screen color picking");
    connectionStatus->setText(connText);
    connectionStatus->setToolTip(connTip);
    if (connectionDot) {
      connectionDot->setToolTip(connTip);
      connectionDot->setColor(
          (picking || !loaded) ? QColor(0xF6, 0x74, 0x00)
          : connected          ? QColor(0x27, 0xAE, 0x60)
                               : StatusDot::neutralColor(palette()));
    }
      if (!hasPendingBoardRender && state == currentSnapshot)
        break;
      prev = currentSnapshot;
    }
    renderingBoard = false;
  }
  QWidget *currentColor() {
    auto m = object(state.value("mixer"));
    const QString currentHex = str(m, "current");
    const QString outputText = str(m, "output");
    return new CurrentColorBanner(
        color(currentHex), currentHex, outputText, headerBarHeight(),
        [this](QString c) {
          if (color(c).isValid())
            send({{"op", "mixer_select"}, {"color", c}});
        });
  }
  QWidget *mixerPage() {
    auto m = object(state.value("mixer"));
    auto page = new QWidget;
    page->setObjectName("mixerPage");
    auto layout = new QVBoxLayout(page);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    auto actions = new QToolBar(page);
    actions->setObjectName("mixerActions");
    actions->setMovable(false);
    actions->setFloatable(false);
    actions->setIconSize(QSize(16, 16));
    actions->setContentsMargins(6, 2, 6, 2);
    actions->setProperty("boardAction", true);
    actions->setToolButtonStyle(
        settings.value("board/toolbarLabels", true).toBool()
            ? Qt::ToolButtonTextBesideIcon
            : Qt::ToolButtonIconOnly);
    const QList<QPair<QString, QString>> actionItems{
        {"Copy", "mixer_copy"},
        {"Add to palette", "palette_add_current"},
        {"Undo", "mixer_undo"},
        {"Redo", "mixer_redo"}};
    for (int idx = 0; idx < actionItems.size(); ++idx) {
      if (idx == 2)
        actions->addSeparator();
      const auto &pair = actionItems[idx];
      const auto icons = actionIcon(pair.second);
      auto icon = themedIcon(icons, actionFallback(pair.second));
      auto action = actions->addAction(icon, pair.first);
      action->setToolTip(actionHint(pair.first, icons.value(0)));
      actions->widgetForAction(action)->setAccessibleName(pair.first);
      connect(action, &QAction::triggered, this,
              [this, pair] { send({{"op", pair.second}}); });
      if (pair.second == "mixer_undo")
        action->setEnabled(m.value("can_undo").toBool());
      if (pair.second == "mixer_redo")
        action->setEnabled(m.value("can_redo").toBool());
    }
    layout->addWidget(actions);
    auto pinnedLine = new Hairline(page);
    pinnedLine->setObjectName("mixerActionsHairline");
    layout->addWidget(pinnedLine);
    layout->addWidget(scroll(mixer()), 1);
    return page;
  }
  QString mixerHsvReadout(double hue, double saturation, double value,
                          int alpha) const {
    const int alphaPct = qRound(qBound(0, alpha, 255) * 100.0 / 255.0);
    return QStringLiteral("H %1°   S %2%   V %3%   A %4%")
        .arg(locale().toString(hue, 'f', 1),
             locale().toString(saturation, 'f', 1),
             locale().toString(value, 'f', 1),
             QString::number(alphaPct));
  }
  QWidget *mixer() {
    auto m = object(state.value("mixer"));
    auto w = new QWidget;
    w->setObjectName("mixerContent");
    auto l = new QVBoxLayout(w);
    l->setContentsMargins(12, 10, 12, 14);
    l->setSpacing(6);
    const QFont monoFont = QFontDatabase::systemFont(QFontDatabase::FixedFont);
    auto channelLabel = [](const QString &text, QWidget *parent) {
      auto lbl = new QLabel(text, parent);
      QPalette pal = lbl->palette();
      const QColor muted = mutedColor(pal, 165);
      pal.setColor(QPalette::WindowText, muted);
      pal.setColor(QPalette::Text, muted);
      lbl->setPalette(pal);
      return lbl;
    };
    auto hsv = array(m, "hsv"), rgba = array(m, "rgba");
    double h = hsv[0].toDouble(), s = hsv[1].toDouble(), v = hsv[2].toDouble();
    auto columns = new QVBoxLayout;
    columns->setObjectName("mixerColumns");
    columns->setSpacing(12);
    auto selectorsBox = new QWidget(w);
    selectorsBox->setObjectName("mixerSelectors");
    selectorsBox->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Preferred);
    auto controls = new QGridLayout(selectorsBox);
    controls->setContentsMargins(0, 0, 0, 0);
    controls->setHorizontalSpacing(12);
    controls->setVerticalSpacing(6);
    auto changed = [this](double h, double s, double v, bool final) {
          spectrumDragging = true;
          send({{"op", "mixer_hsv"}, {"h", h}, {"s", s}, {"v", v}});
          if (final) {
            spectrumDragging = false;
            auto previous = state;
            previous.remove("mixer");
            renderBoard(previous);
          }
        };
    mixerSpectrum = new Spectrum(h, s, v, changed, selectorsBox);
    mixerSpectrum->setObjectName("mixerSpectrum");
    mixerWheel = new MixerWheel(h, s, v, changed, selectorsBox);
    mixerWheel->setObjectName("mixerWheel");
    mixerReadout = new QLabel(
        mixerHsvReadout(h, s, v, rgba.size() == 4 ? rgba[3].toInt() : 255),
        selectorsBox);
    mixerReadout->setObjectName("mixerHsvReadout");
    QFont readoutFont = monoFont;
    if (readoutFont.pointSizeF() > 0)
      readoutFont.setPointSizeF(qMax(8.5, readoutFont.pointSizeF() - 1.0));
    else if (readoutFont.pixelSize() > 0)
      readoutFont.setPixelSize(qMax(10, readoutFont.pixelSize() - 1));
    mixerReadout->setFont(readoutFont);
    QPalette readoutPal = mixerReadout->palette();
    const QColor readoutFg = mutedColor(readoutPal, 200);
    readoutPal.setColor(QPalette::WindowText, readoutFg);
    readoutPal.setColor(QPalette::Text, readoutFg);
    mixerReadout->setPalette(readoutPal);
    mixerReadout->setVisible(shortMixer());
    controls->addWidget(mixerSpectrum, 0, 0, Qt::AlignVCenter);
    controls->addWidget(mixerWheel, 0, 1, Qt::AlignVCenter);
    controls->addWidget(mixerReadout, 1, 0, 1, 2, Qt::AlignRight | Qt::AlignTop);
    columns->addWidget(selectorsBox, 0, Qt::AlignLeft | Qt::AlignTop);
    l->addLayout(columns);
    auto inputs = new QWidget(w);
    inputs->setObjectName("mixerInputs");
    inputs->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    inputs->setMaximumWidth(460);
    auto form = new QFormLayout(inputs);
    form->setContentsMargins(0, 0, 0, 0);
    form->setHorizontalSpacing(8);
    form->setVerticalSpacing(5);
    form->setLabelAlignment(Qt::AlignRight | Qt::AlignVCenter);
    form->setRowWrapPolicy(QFormLayout::WrapLongRows);
    form->setFieldGrowthPolicy(QFormLayout::AllNonFixedFieldsGrow);
    auto hbox = new QWidget(inputs);
    hbox->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    auto hl = new QHBoxLayout(hbox);
    hl->setContentsMargins(0, 0, 0, 0);
    hl->setSpacing(4);
    auto hh = decimal("Hue", h, 0, 360, hbox),
         ss = decimal("Saturation", s, 0, 100, hbox),
         vv = decimal("Value", v, 0, 100, hbox);
    for (auto x : {hh, ss, vv}) {
      if (x != hh)
        hl->addSpacing(4);
      x->setDecimals(1);
      x->setValue(x == hh ? h : x == ss ? s : v);
      x->setSingleStep(.1);
      x->setSuffix(x == hh ? QString::fromUtf8("°") : "%");
      x->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
      auto label = channelLabel(x == hh ? "H" : x == ss ? "S" : "V", hbox);
      label->setBuddy(x);
      hl->addWidget(label);
      hl->addWidget(x, 1);
    }
    const double displayedH = hh->value(), displayedS = ss->value(),
                 displayedV = vv->value();
    for (auto x : {hh, ss, vv}) {
      connect(x, &QDoubleSpinBox::editingFinished, this,
              [this, hh, ss, vv, h, s, v, displayedH, displayedS, displayedV] {
                bool changedH = hh->value() != displayedH;
                bool changedS = ss->value() != displayedS;
                bool changedV = vv->value() != displayedV;
                if (!changedH && !changedS && !changedV)
                  return;
                send({{"op", "mixer_hsv"},
                      {"h", changedH ? hh->value() : h},
                      {"s", changedS ? ss->value() : s},
                      {"v", changedV ? vv->value() : v}});
              });
    }
    form->addRow("HSV:", hbox);
    auto rw = new QWidget(inputs);
    rw->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    auto rl = new QHBoxLayout(rw);
    rl->setContentsMargins(0, 0, 0, 0);
    rl->setSpacing(4);
    QList<QSpinBox *> channels;
    const char *names[] = {"Red", "Green", "Blue", "Alpha"};
    for (int i = 0; i < 4; i++) {
      if (i > 0)
        rl->addSpacing(2);
      auto x = spin(names[i], rgba[i].toInt(), 0, 255, rw);
      x->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
      channels << x;
      auto label = channelLabel(QString(names[i]).left(1), rw);
      label->setBuddy(x);
      rl->addWidget(label);
      rl->addWidget(x, 1);
    }
    for (auto x : channels)
      connect(x, &QSpinBox::editingFinished, this, [this, channels] {
        send({{"op", "mixer_rgba"},
              {"r", channels[0]->value()},
              {"g", channels[1]->value()},
              {"b", channels[2]->value()},
              {"a", channels[3]->value()}});
      });
    form->addRow("RGBA:", rw);
    auto fields = array(m, "fields");
    auto formatRow = new QWidget(inputs);
    formatRow->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    auto formatLayout = new QHBoxLayout(formatRow);
    formatLayout->setContentsMargins(0, 0, 0, 0);
    formatLayout->setSpacing(6);
    auto format = new QComboBox(formatRow);
    format->setAccessibleName("Color format");
    format->addItems({"HEX", "RGB", "RGBA", "HSL", "HSV"});
    format->setCurrentIndex(mixerFormat);
    format->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto value = line("Color value", fields[mixerFormat].toString(), formatRow);
    value->setFont(monoFont);
    value->setMinimumWidth(
        value->fontMetrics().horizontalAdvance(QString(16, QLatin1Char('0'))) + 16);
    value->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    auto apply = [this, value] {
      value->setModified(false);
      send({{"op", "mixer_apply"}, {"index", mixerFormat}, {"text", value->text()}});
    };
    connect(format, &QComboBox::currentIndexChanged, this,
            [this, value, fields](int index) {
              mixerFormat = index;
              value->setText(fields[index].toString());
            });
    connect(value, &QLineEdit::returnPressed, this, apply);
    auto applyBtn = button("Apply", formatRow, apply);
    applyBtn->setIcon(themedIcon({"dialog-ok-apply-symbolic", "dialog-ok-apply"},
                                 QStyle::SP_DialogApplyButton));
    applyBtn->setIconSize(QSize(16, 16));
    applyBtn->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    formatLayout->addWidget(format, 0);
    formatLayout->addWidget(value, 1);
    formatLayout->addWidget(applyBtn, 0);
    auto formatLabel = new QLabel("Format:", inputs);
    formatLabel->setBuddy(format);
    form->addRow(formatLabel, formatRow);
    auto cmykLabel = new QLabel(str(m, "cmyk"), inputs);
    cmykLabel->setTextInteractionFlags(Qt::TextSelectableByMouse);
    form->addRow("CMYK:", cmykLabel);
    columns->addWidget(inputs, 1, Qt::AlignTop);
    setTabOrder(mixerSpectrum, mixerWheel);
    l->addWidget(new Hairline(w));
    auto mixHeader = new QWidget(w);
    mixHeader->setObjectName("mixerMixHeader");
    mixHeader->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Fixed);
    auto mixHeaderLayout = new QBoxLayout(QBoxLayout::LeftToRight, mixHeader);
    mixHeaderLayout->setContentsMargins(0, 0, 0, 0);
    mixHeaderLayout->setSpacing(10);
    auto mixLeft = new QWidget(mixHeader);
    mixLeft->setObjectName("mixerMixLeft");
    auto mixLeftLayout = new QHBoxLayout(mixLeft);
    mixLeftLayout->setContentsMargins(0, 0, 0, 0);
    mixLeftLayout->setSpacing(6);
    auto mixTitle = new QLabel("Color mixing", mixLeft);
    QFont mixTitleFont = mixTitle->font();
    mixTitleFont.setWeight(QFont::DemiBold);
    mixTitle->setFont(mixTitleFont);
    mixLeftLayout->addWidget(mixTitle, 0, Qt::AlignVCenter);
    mixLeftLayout->addSpacing(4);
    auto spaceLabel = channelLabel("Space:", mixLeft);
    auto space = new QComboBox(mixLeft);
    space->setAccessibleName("Mixing color space");
    space->addItem("sRGB", "srgb");
    space->addItem("OKLCH", "oklch");
    space->setCurrentIndex(qMax(0, space->findData(str(m, "space"))));
    space->setSizeAdjustPolicy(QComboBox::AdjustToContents);
    space->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Fixed);
    spaceLabel->setBuddy(space);
    connect(space, &QComboBox::currentIndexChanged, this, [this, space] {
      send({{"op", "mixer_space"}, {"space", space->currentData().toString()}});
    });
    mixLeftLayout->addWidget(spaceLabel, 0, Qt::AlignVCenter);
    mixLeftLayout->addWidget(space, 0, Qt::AlignVCenter);
    auto mixRight = new QWidget(mixHeader);
    mixRight->setObjectName("mixerMixRight");
    auto mixRightLayout = new QHBoxLayout(mixRight);
    mixRightLayout->setContentsMargins(0, 0, 0, 0);
    mixRightLayout->setSpacing(6);
    auto addSourceBtn = toolButton(
        "Add source", actionIcon("mixer_source_add"), mixRight,
        [this] { send({{"op", "mixer_source_add"}}); },
        actionFallback("mixer_source_add"));
    addSourceBtn->setAutoRaise(true);
    addSourceBtn->setIconSize(QSize(16, 16));
    addSourceBtn->setToolTip("Add a color mixing source");
    auto mixBtn = button("Mix", mixRight,
                         [this] { send({{"op", "mixer_mix"}}); });
    mixBtn->setIcon(themedIcon(actionIcon("mixer_mix"),
                               actionFallback("mixer_mix")));
    mixBtn->setIconSize(QSize(16, 16));
    mixBtn->setToolTip("Mix sources into the current color");
    mixRightLayout->addWidget(addSourceBtn, 0, Qt::AlignVCenter);
    mixRightLayout->addWidget(mixBtn, 0, Qt::AlignVCenter);
    mixHeaderLayout->addWidget(mixLeft, 0, Qt::AlignVCenter);
    mixHeaderLayout->addWidget(mixRight, 0, Qt::AlignVCenter);
    l->addWidget(mixHeader, 0, Qt::AlignLeft);
    int i = 0;
    for (auto value : array(m, "sources")) {
      auto o = value.toObject();
      int index = i++;
      auto row = new QWidget(w);
      auto r = new QHBoxLayout(row);
      r->setContentsMargins(0, 0, 0, 0);
      r->setSpacing(8);
      const QString initialColor = str(o, "color");
      const double initialWeight = o.value("weight").toDouble();
      auto source = new Swatch(color(initialColor),
                               QStringLiteral("Source %1").arg(index + 1), row);
      source->setText(QString());
      source->setFixedSize(34, 24);
      source->setTileMode(true, false);
      source->setToolTip(
          QStringLiteral("Source %1 — drag or drop a color").arg(index + 1));
      auto c = line("Source color", initialColor, row);
      c->setFont(monoFont);
      const int hexFieldW =
          c->fontMetrics().horizontalAdvance(QStringLiteral("#00000000")) + 24;
      c->setMinimumWidth(hexFieldW);
      c->setMaximumWidth(hexFieldW + 8);
      c->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Fixed);
      auto weightBox = new QWidget(row);
      auto weightLayout = new QHBoxLayout(weightBox);
      weightLayout->setContentsMargins(0, 0, 0, 0);
      weightLayout->setSpacing(4);
      auto weightLabel = channelLabel("Weight:", weightBox);
      auto weight = decimal("Source weight", initialWeight, 0, 1, weightBox);
      weight->setMaximumWidth(weight->sizeHint().width());
      weightLabel->setBuddy(weight);
      weightLayout->addWidget(weightLabel);
      weightLayout->addWidget(weight);
      source->paletteDrop(-1, [this, index, weight](int, QString c) {
        if (color(c).isValid())
          send({{"op", "mixer_source"},
                {"index", index},
                {"color", c},
                {"weight", weight->value()}});
      });
      r->addWidget(source, 0, Qt::AlignVCenter);
      r->addWidget(c, 0, Qt::AlignVCenter);
      r->addWidget(weightBox, 0, Qt::AlignVCenter);
      const double displayedWeight = weight->value();
      auto apply = [this, index, c, weight, initialColor, initialWeight,
                    displayedWeight] {
        const bool colorChanged = c->text() != initialColor;
        const bool weightChanged = weight->value() != displayedWeight;
        if (!colorChanged && !weightChanged)
          return;
        send({{"op", "mixer_source"},
              {"index", index},
              {"color", colorChanged ? c->text() : initialColor},
              {"weight", weightChanged ? weight->value() : initialWeight}});
      };
      connect(c, &QLineEdit::editingFinished, this, apply);
      connect(weight, &QDoubleSpinBox::editingFinished, this, apply);
      auto removeBtn = new QToolButton(row);
      removeBtn->setAutoRaise(true);
      removeBtn->setIcon(themedIcon(
          {"list-remove-symbolic", "edit-delete-symbolic", "list-remove"},
          QStyle::SP_TrashIcon));
      removeBtn->setIconSize(QSize(16, 16));
      removeBtn->setFixedSize(26, 26);
      removeBtn->setAccessibleName("Remove");
      removeBtn->setToolTip("Remove source");
      connect(removeBtn, &QToolButton::clicked, this, [this, index] {
        send({{"op", "mixer_source_remove"}, {"index", index}});
      });
      r->addWidget(removeBtn, 0, Qt::AlignVCenter);
      r->addStretch(1);
      l->addWidget(row);
    }
    auto error = new QLabel(str(m, "error"), w);
    error->setWordWrap(true);
    error->setVisible(!error->text().isEmpty());
    l->addWidget(error);
    l->addStretch();
    return w;
  }
  void layoutMixerContent(QWidget *mixer, const QSize &viewportSize) {
    if (!mixer || mixer->objectName() != "mixerContent")
      return;
    auto columns = mixer->findChild<QBoxLayout *>("mixerColumns");
    auto selectorsBox = mixer->findChild<QWidget *>("mixerSelectors");
    auto selectors =
        selectorsBox ? qobject_cast<QGridLayout *>(selectorsBox->layout())
                     : nullptr;
    auto inputs = mixer->findChild<QWidget *>("mixerInputs");
    auto spectrum =
        selectorsBox ? selectorsBox->findChild<QWidget *>("mixerSpectrum")
                     : nullptr;
    auto wheel =
        selectorsBox ? selectorsBox->findChild<QWidget *>("mixerWheel")
                     : nullptr;
    auto readout =
        selectorsBox ? selectorsBox->findChild<QLabel *>("mixerHsvReadout")
                     : nullptr;
    const auto margins = mixer->layout()->contentsMargins();
    const int availW = viewportSize.width() - margins.left() - margins.right();
    if (columns && selectorsBox && selectors && inputs && spectrum && wheel) {
      const int widePlane = 164;
      const int wideWheel = 164;
      const int compactSide = 136;
      const int wideSelSpacing = 12;
      const int wideColSpacing = 14;
      const bool shortView = shortMixer();
      const bool wide = !shortView &&
                        availW >= widePlane + wideWheel +
                                      inputs->sizeHint().width() +
                                      wideSelSpacing + wideColSpacing;
      const int side = shortView ? compactSide : wide ? widePlane : 156;
      spectrum->setFixedSize(side, side);
      const int wheelSide = shortView ? compactSide : wide ? wideWheel : 156;
      wheel->setFixedSize(wheelSide, wheelSide);
      selectors->setHorizontalSpacing(wide ? wideSelSpacing : 16);
      if (readout)
        readout->setVisible(shortView);
      columns->setDirection(wide ? QBoxLayout::LeftToRight
                                 : QBoxLayout::TopToBottom);
      columns->setAlignment(selectorsBox,
                            wide ? (Qt::AlignLeft | Qt::AlignVCenter)
                                 : (Qt::AlignLeft | Qt::AlignTop));
      columns->setAlignment(inputs, wide ? Qt::AlignVCenter : Qt::AlignTop);
      if (wide) {
        columns->setSpacing(wideColSpacing);
      } else if (shortView) {
        const int readoutH =
            readout ? readout->sizeHint().height() + selectors->verticalSpacing()
                    : 0;
        const int selectorsBottom = margins.top() + compactSide + readoutH;
        const int foldGap = viewportSize.height() - selectorsBottom + 2;
        columns->setSpacing(qBound(12, foldGap, 48));
      } else {
        columns->setSpacing(12);
      }
      selectorsBox->updateGeometry();
    }
    if (auto mixHeader = mixer->findChild<QWidget *>("mixerMixHeader")) {
      auto mixLayout = qobject_cast<QBoxLayout *>(mixHeader->layout());
      auto mixLeft = mixHeader->findChild<QWidget *>("mixerMixLeft");
      auto mixRight = mixHeader->findChild<QWidget *>("mixerMixRight");
      if (mixLayout && mixLeft && mixRight) {
        const bool singleRow =
            availW >= mixLeft->sizeHint().width() + mixRight->sizeHint().width() + 10;
        mixLayout->setDirection(singleRow ? QBoxLayout::LeftToRight
                                          : QBoxLayout::TopToBottom);
        mixLayout->setSpacing(singleRow ? 10 : 6);
        mixLayout->setAlignment(mixLeft,
                                singleRow ? Qt::AlignVCenter : Qt::AlignLeft);
        mixLayout->setAlignment(mixRight,
                                singleRow ? Qt::AlignVCenter : Qt::AlignLeft);
        mixHeader->updateGeometry();
      }
    }
  }
  void alignHeaderToolbar(QWidget *page, const char *barName) {
    if (!page || !tabs || !tabs->isAncestorOf(page))
      return;
    if (auto *bar = page->findChild<QToolBar *>(QLatin1String(barName))) {
      const int topShift = qMax(0, -page->mapTo(tabs, QPoint(0, 0)).y());
      const int targetH = headerBarHeight() + topShift;
      if (bar->minimumHeight() != targetH || bar->maximumHeight() != targetH) {
        bar->setFixedHeight(targetH);
        bar->setContentsMargins(6, topShift, 6, 0);
      }
    }
  }
  void layoutPaletteNameRow(QWidget *nameRow, int width) {
    if (!nameRow || nameRow->objectName() != "paletteNameRow")
      return;
    auto box = qobject_cast<QBoxLayout *>(nameRow->layout());
    auto lock = nameRow->findChild<QCheckBox *>();
    if (!box || !lock)
      return;
    const bool stacked = width > 0 && width < (120 + 12 + lock->sizeHint().width());
    if (nameRow->property("stackedMode").isValid() &&
        nameRow->property("stackedMode").toBool() == stacked)
      return;
    nameRow->setProperty("stackedMode", stacked);
    box->setDirection(stacked ? QBoxLayout::TopToBottom : QBoxLayout::LeftToRight);
    box->setSpacing(stacked ? 4 : 12);
    nameRow->updateGeometry();
    if (auto *p = nameRow->parentWidget()) {
      p->updateGeometry();
      if (p->layout())
        p->layout()->invalidate();
    }
  }
  void layoutPaletteGenerator(QWidget *gen, int width) {
    if (!gen || gen->objectName() != "paletteGenerator" || width <= 0)
      return;
    bool changed = false;
    auto genRoot = qobject_cast<QVBoxLayout *>(gen->layout());
    auto columns = gen->findChild<QBoxLayout *>("paletteGeneratorColumns");
    auto parameters =
        gen->findChild<QWidget *>("paletteParameters", Qt::FindDirectChildrenOnly);
    auto selector =
        gen->findChild<QWidget *>("paletteSelector", Qt::FindDirectChildrenOnly);
    if (columns && parameters && selector) {
      const bool compact = width < 620;
      if (!gen->property("compactMode").isValid() ||
          gen->property("compactMode").toBool() != compact) {
        changed = true;
        gen->setProperty("compactMode", compact);
        if (genRoot)
          genRoot->setSpacing(compact ? 8 : 10);
        if (auto *parentWidget = gen->parentWidget()) {
          if (auto *genSection =
                  parentWidget->findChild<QVBoxLayout *>("paletteGenSection"))
            genSection->setSpacing(compact ? 2 : 6);
        }
        while (columns->count() > 0)
          delete columns->takeAt(0);
        columns->setDirection(compact ? QBoxLayout::TopToBottom
                                      : QBoxLayout::LeftToRight);
        columns->setSpacing(compact ? 8 : 28);
        if (compact) {
          parameters->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
          selector->setMinimumWidth(0);
          selector->setMaximumWidth(QWIDGETSIZE_MAX);
          selector->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
          if (auto *sl = qobject_cast<QVBoxLayout *>(selector->layout()))
            sl->setSpacing(4);
          if (auto *wheel = selector->findChild<QWidget *>("paletteWheel"))
            wheel->setFixedSize(124, 124);
          if (auto *strip = selector->findChild<QWidget *>("palettePreviewStrip"))
            strip->setFixedHeight(32);
          columns->addWidget(selector, 0);
          columns->addWidget(parameters, 0);
        } else {
          parameters->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Preferred);
          selector->setMinimumWidth(220);
          selector->setMaximumWidth(QWIDGETSIZE_MAX);
          selector->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
          if (auto *sl = qobject_cast<QVBoxLayout *>(selector->layout()))
            sl->setSpacing(10);
          if (auto *wheel = selector->findChild<QWidget *>("paletteWheel"))
            wheel->setFixedSize(176, 176);
          if (auto *strip = selector->findChild<QWidget *>("palettePreviewStrip"))
            strip->setFixedHeight(38);
          columns->addWidget(parameters, 0, Qt::AlignTop);
          columns->addWidget(selector, 1, Qt::AlignTop);
        }
      }
    }
    auto footer = gen->findChild<QWidget *>("savedGenerationFooter",
                                            Qt::FindDirectChildrenOnly);
    auto footerLayout = footer ? qobject_cast<QBoxLayout *>(footer->layout()) : nullptr;
    auto savedLabel =
        footer ? footer->findChild<QLabel *>("savedGenerationLabel") : nullptr;
    auto savedRow =
        footer ? footer->findChild<QWidget *>("savedGenerationRow") : nullptr;
    if (footer && footerLayout && savedLabel && savedRow) {
      const bool stackedFooter = width < 520;
      if (!gen->property("stackedFooterMode").isValid() ||
          gen->property("stackedFooterMode").toBool() != stackedFooter) {
        changed = true;
        gen->setProperty("stackedFooterMode", stackedFooter);
        footerLayout->setDirection(stackedFooter ? QBoxLayout::TopToBottom
                                                 : QBoxLayout::LeftToRight);
        footerLayout->setSpacing(stackedFooter ? 4 : 8);
        footerLayout->setAlignment(
            savedLabel, stackedFooter ? Qt::AlignLeft : Qt::AlignVCenter);
        footerLayout->setStretchFactor(savedLabel, 0);
        footerLayout->setStretchFactor(savedRow, stackedFooter ? 0 : 1);
        footer->updateGeometry();
      }
    }
    if (changed) {
      gen->updateGeometry();
      if (auto *p = gen->parentWidget()) {
        p->updateGeometry();
        if (p->layout())
          p->layout()->invalidate();
      }
    }
  }
  QWidget *palettePage();
  QWidget *createThemePage();
  QWidget *buildThemeBody(Scene *reuse = nullptr);
  void refreshThemeContent(Scene *reuse = nullptr);
  void syncThemeProjectDialog();
  void layoutThemePage(QWidget *page, int width, bool initialize = false);
  void renderConfig();
  void renderHistory();

protected:
  void changeEvent(QEvent *event) override {
    QWidget::changeEvent(event);
    if (event->type() == QEvent::PaletteChange ||
        event->type() == QEvent::StyleChange)
      updateNavButtonStyles();
  }
  void resizeEvent(QResizeEvent *event) override {
    QWidget::resizeEvent(event);
    sizeHistorySidebar();
    sizePinnedPalette();
    if (tabs && tabs->widget(0)) {
      if (auto *scroll = tabs->widget(0)->findChild<QScrollArea *>())
        layoutMixerContent(scroll->widget(), scroll->viewport()->size());
    }
  }
  bool eventFilter(QObject *watched, QEvent *event) override {
    if (event->type() == QEvent::Show || event->type() == QEvent::Move) {
      if (auto page = qobject_cast<QWidget *>(watched)) {
        if (page->objectName() == "palettePage")
          alignHeaderToolbar(page, "paletteActions");
        else if (page->objectName() == "themePage")
          alignHeaderToolbar(page, "themeActions");
      }
    }
    if (event->type() == QEvent::Resize) {
      if (watched == historyPanel)
        updateHistoryEmptyState(historyPanel->width());
      auto page = qobject_cast<QWidget *>(watched);
      if (page && page->objectName() == "palettePage")
        alignHeaderToolbar(page, "paletteActions");
      auto scroll = page ? qobject_cast<QScrollArea *>(page->parentWidget()) : nullptr;
      if (scroll && page == scroll->viewport() && scroll->widget() &&
          scroll->widget()->objectName() == "mixerContent") {
        layoutMixerContent(scroll->widget(),
                           static_cast<QResizeEvent *>(event)->size());
      }
      if (page && page->objectName() == "paletteNameRow")
        layoutPaletteNameRow(page, static_cast<QResizeEvent *>(event)->size().width());
      if (page && page->objectName() == "paletteGenerator")
        layoutPaletteGenerator(page, static_cast<QResizeEvent *>(event)->size().width());
      if (page && page->objectName() == "themePage") {
        alignHeaderToolbar(page, "themeActions");
        sizePinnedPalette();
        layoutThemePage(page, static_cast<QResizeEvent *>(event)->size().width());
      }
    }
    if (mode == "history" && event->type() == QEvent::KeyPress) {
      auto key = static_cast<QKeyEvent *>(event)->key();
      int i = historyButtons.indexOf(qobject_cast<QPushButton *>(watched));
      if (i >= 0) {
        int columns = qMax(1, state.value("columns").toInt(4)),
            page = columns * qMax(1, state.value("rows").toInt(3)), next = i;
        if (key == Qt::Key_Left)
          --next;
        else if (key == Qt::Key_Right)
          ++next;
        else if (key == Qt::Key_Up)
          next -= columns;
        else if (key == Qt::Key_Down)
          next += columns;
        else if (key == Qt::Key_PageUp)
          next -= page;
        else if (key == Qt::Key_PageDown)
          next += page;
        else if (key == Qt::Key_Home)
          next = 0;
        else if (key == Qt::Key_End)
          next = historyButtons.size() - 1;
        else if (key == Qt::Key_Return || key == Qt::Key_Enter) {
          historyButtons[i]->click();
          return true;
        } else if (key == Qt::Key_Escape) {
          close();
          return true;
        } else
          return QWidget::eventFilter(watched, event);
        const int clamped = qBound(0, next, int(historyButtons.size()) - 1);
        historyButtons[clamped]->setFocus();
        if (historyScroll)
          historyScroll->ensureWidgetVisible(historyButtons[clamped], 12, 12);
        return true;
      }
    }
    return QWidget::eventFilter(watched, event);
  }
  void closeEvent(QCloseEvent *event) override {
    auto project = object(object(state.value("theme")).value("project"));
    if (mode == "board" && project.value("dirty").toBool() &&
        QMessageBox::question(this, "Unsaved theme",
                              "Discard unsaved theme changes and close?") !=
            QMessageBox::Yes) {
      event->ignore();
      return;
    }
    QWidget::closeEvent(event);
  }

public:
  void smoke() {
    QString dir = qEnvironmentVariable("SWATCH_BOARD_QT_SMOKE_DIR");
    if (dir.isEmpty())
      return;
    QDir().mkpath(dir);
    QTimer::singleShot(350, this, [this, dir] {
      QJsonArray failures;
      auto check = [&](bool ok, const QString &what) {
        if (!ok)
          failures.append(what);
      };
      auto settleLayout = [&] {
        if (layout())
          layout()->activate();
        QCoreApplication::sendPostedEvents(nullptr, QEvent::LayoutRequest);
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        if (layout())
          layout()->activate();
      };
      auto shot = [&](const QString &name) {
        settleLayout();
        check(grab().save(dir + "/" + name + ".png"), "Screenshot " + name);
      };
      auto click = [&](QWidget *root, const QString &name) {
        for (auto b : root->findChildren<QAbstractButton *>())
          if (b->accessibleName() == name) {
            b->click();
            return true;
          }
        failures.append("Missing button " + name);
        return false;
      };
      auto edit = [&](QWidget *root, const QString &name,
                      const QString &value) {
        for (auto e : root->findChildren<QLineEdit *>())
          if (e->accessibleName() == name) {
            e->setText(value);
            return true;
          }
        failures.append("Missing field " + name);
        return false;
      };
      if (mode == "board" && tabs) {
        tabs->setCurrentIndex(0);
        check(!undoShortcut->isEnabled() && !redoShortcut->isEnabled(),
              "Mixer shortcuts disabled without history");
        auto selectFormat = [&](const QString &name) {
          for (auto combo : tabs->widget(0)->findChildren<QComboBox *>())
            if (combo->accessibleName() == "Color format") {
              combo->setCurrentText(name);
              return;
            }
          failures.append("Missing color format choice");
        };
        edit(tabs->widget(0), "Color value", "#01020342");
        click(tabs->widget(0), "Apply");
        check(str(object(state.value("mixer")), "current") == "#01020342",
              "Apply HEX");
        selectFormat("RGB");
        edit(tabs->widget(0), "Color value", "rgb(4, 5, 6)");
        click(tabs->widget(0), "Apply");
        check(str(object(state.value("mixer")), "current") == "#04050642",
              "RGB preserves alpha");
        for (auto combo : tabs->widget(0)->findChildren<QComboBox *>())
          if (combo->accessibleName() == "Color format")
            check(combo->currentText() == "RGB", "Format retained after refresh");
        auto key = [](QWidget *target, int code, Qt::KeyboardModifiers modifiers) {
          QKeyEvent press(QEvent::KeyPress, code, modifiers, code == Qt::Key_X ? "x" : QString());
          QApplication::sendEvent(target, &press);
          QKeyEvent release(QEvent::KeyRelease, code, modifiers);
          QApplication::sendEvent(target, &release);
        };
        auto page = tabs->widget(0);
        QAbstractButton *undoButton = nullptr, *redoButton = nullptr, *applyButton = nullptr;
        for (auto b : page->findChildren<QAbstractButton *>()) {
          if (b->accessibleName() == "Undo") undoButton = b;
          if (b->accessibleName() == "Redo") redoButton = b;
          if (b->accessibleName() == "Apply") applyButton = b;
        }
        check(undoShortcut->isEnabled() && !redoShortcut->isEnabled(),
              "Mixer shortcut availability after editing");
        auto undoHint = QKeySequence(QKeySequence::Undo).toString(QKeySequence::NativeText);
        auto redoHint = QKeySequence(QKeySequence::Redo).toString(QKeySequence::NativeText);
        check(undoButton && undoButton->isEnabled() && undoButton->toolTip().contains(undoHint),
              "Enabled Undo button shows shortcut hint");
        check(redoButton && !redoButton->isEnabled() && redoButton->toolTip().contains(redoHint),
              "Disabled Redo button shows shortcut hint");
        check(applyButton, "Apply shortcut target available");
        if (undoButton && applyButton) {
          shot("shortcut-before-undo");
          auto pos = undoButton->rect().center();
          QHelpEvent hover(QEvent::ToolTip, pos, undoButton->mapToGlobal(pos));
          QApplication::sendEvent(undoButton, &hover);
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
          for (auto widget : QApplication::topLevelWidgets())
            if (widget->windowType() == Qt::ToolTip && widget->isVisible())
              check(widget->grab().save(dir + "/shortcut-tooltip.png"), "Screenshot undo tooltip");
          QToolTip::hideText();
          applyButton->setFocus();
          key(applyButton, Qt::Key_Z, Qt::ControlModifier);
          check(str(object(state.value("mixer")), "current") == "#01020342",
                "Undo keyboard shortcut");
          check(redoShortcut->isEnabled(), "Mixer redo available after undo");
          shot("shortcut-after-undo");
          auto target = QApplication::focusWidget();
          key(target ? target : this, Qt::Key_Z, Qt::ControlModifier | Qt::ShiftModifier);
          check(str(object(state.value("mixer")), "current") == "#04050642",
                "Redo keyboard shortcut");
          check(!redoShortcut->isEnabled(), "Mixer redo exhausted");
          shot("shortcut-after-redo");
        }
        click(tabs->widget(0), "Undo");
        check(str(object(state.value("mixer")), "current") == "#01020342",
              "Undo");
        click(tabs->widget(0), "Redo");
        check(str(object(state.value("mixer")), "current") == "#04050642",
              "Redo");
        QLineEdit *field = nullptr;
        for (auto input : tabs->widget(0)->findChildren<QLineEdit *>())
          if (input->accessibleName() == "Color value") field = input;
        check(field, "Editable field available");
        if (field) {
          field->setFocus();
          auto original = field->text();
          field->selectAll();
          key(field, Qt::Key_X, Qt::NoModifier);
          check(field->text() == "x", "Editable field receives typed text");
          key(field, Qt::Key_Z, Qt::ControlModifier);
          check(str(object(state.value("mixer")), "current") == "#04050642" && field->text() == original,
                "Editable field handles native undo");
          key(field, Qt::Key_Z, Qt::ControlModifier | Qt::ShiftModifier);
          check(str(object(state.value("mixer")), "current") == "#04050642" && field->text() == "x",
                "Editable field handles native redo");
          key(field, Qt::Key_Z, Qt::ControlModifier);
        }
        for (auto example : QList<QPair<QString, QString>>{
                 {"RGBA", "rgba(255, 0, 0, 0.5)"},
                 {"HSL", "hsl(120, 100%, 50%)"},
                 {"HSV", "hsv(240, 100%, 100%)"}}) {
          selectFormat(example.first);
          edit(tabs->widget(0), "Color value", example.second);
          for (auto field : tabs->widget(0)->findChildren<QLineEdit *>())
            if (field->accessibleName() == "Color value") {
              QKeyEvent enter(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier);
              QApplication::sendEvent(field, &enter);
              break;
            }
          auto current = str(object(state.value("mixer")), "current");
          check(current.startsWith(example.first == "RGBA" ? "#FF0000" :
                                   example.first == "HSL" ? "#00FF00" : "#0000FF"),
                example.first + " applies with Enter");
          check(current.endsWith("80"), example.first + " alpha");
        }
        send({{"op", "mixer_hsv"}, {"h", 123.456}, {"s", 67.891}, {"v", 42.345}});
        auto hsvBefore = array(object(state.value("mixer")), "hsv");
        auto findHsv = [&](const QString &name) -> QDoubleSpinBox * {
          for (auto control : tabs->widget(0)->findChildren<QDoubleSpinBox *>())
            if (control->accessibleName() == name)
              return control;
          return nullptr;
        };
        auto hue = findHsv("Hue");
        check(hue, "Hue spinbox available");
        if (hue) {
          QMetaObject::invokeMethod(hue, "editingFinished");
          auto hsvAfter = array(object(state.value("mixer")), "hsv");
          check(hsvBefore == hsvAfter, "Unchanged HSV focus preserves precision");
          hue->setValue(124.5);
          QMetaObject::invokeMethod(hue, "editingFinished");
          hsvAfter = array(object(state.value("mixer")), "hsv");
          check(qAbs(hsvAfter[0].toDouble() - 124.5) < .001 &&
                    qAbs(hsvAfter[1].toDouble() - hsvBefore[1].toDouble()) < .0001 &&
                    qAbs(hsvAfter[2].toDouble() - hsvBefore[2].toDouble()) < .0001,
                "Fractional HSV edit preserves untouched channels");
        }
        send({{"op", "mixer_hsv"}, {"h", 0}, {"s", 100}, {"v", 100}});
        check(mixerWheel && mixerSpectrum, "Mixer wheel and gradient available");
        if (mixerWheel && mixerSpectrum) {
          QPointF top(mixerWheel->width() / 2.0, 8);
          QMouseEvent wheelPress(QEvent::MouseButtonPress, top, Qt::LeftButton,
                                 Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(mixerWheel, &wheelPress);
          auto hsv = array(object(state.value("mixer")), "hsv");
          check(hsv.size() == 3 && qAbs(hsv[0].toDouble() - 90) < 2 &&
                    qAbs(mixerSpectrum->hue() - hsv[0].toDouble()) < .01 &&
                    qAbs(mixerSpectrum->saturation() - hsv[1].toDouble()) < .01,
                "Wheel updates gradient while dragging");
          QMouseEvent wheelRelease(QEvent::MouseButtonRelease, top, Qt::LeftButton,
                                   Qt::NoButton, Qt::NoModifier);
          QApplication::sendEvent(mixerWheel, &wheelRelease);
          QPointF point(mixerSpectrum->width() / 4.0, mixerSpectrum->height() / 4.0);
          QMouseEvent gradientPress(QEvent::MouseButtonPress, point, Qt::LeftButton,
                                    Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(mixerSpectrum, &gradientPress);
          hsv = array(object(state.value("mixer")), "hsv");
          check(hsv.size() == 3 && qAbs(hsv[1].toDouble() - 25) < 2 &&
                    qAbs(hsv[2].toDouble() - 75) < 2 &&
                    qAbs(mixerWheel->hue() - hsv[0].toDouble()) < .01 &&
                    qAbs(mixerWheel->saturation() - hsv[1].toDouble()) < .01,
                "Gradient updates wheel while dragging");
          check(str(object(state.value("mixer")), "current").endsWith("80"),
                "Mixer selectors preserve alpha");
          QMouseEvent gradientRelease(QEvent::MouseButtonRelease, point, Qt::LeftButton,
                                      Qt::NoButton, Qt::NoModifier);
          QApplication::sendEvent(mixerSpectrum, &gradientRelease);
          const auto beforeKeys = array(object(state.value("mixer")), "hsv");
          key(mixerSpectrum, Qt::Key_Right, Qt::NoModifier);
          key(mixerSpectrum, Qt::Key_Up, Qt::NoModifier);
          hsv = array(object(state.value("mixer")), "hsv");
          check(qAbs(hsv[0].toDouble() - beforeKeys[0].toDouble()) < .001 &&
                    qAbs(hsv[1].toDouble() - beforeKeys[1].toDouble() - 1) < .001 &&
                    qAbs(hsv[2].toDouble() - beforeKeys[2].toDouble() - 1) < .001 &&
                    str(object(state.value("mixer")), "current").endsWith("80"),
                "Spectrum arrow keys adjust saturation and value without changing hue or alpha");
          send({{"op", "mixer_hsv"}, {"h", 210}, {"s", 80}, {"v", 0}});
          QPointF right(mixerWheel->width() - 8.0, mixerWheel->height() / 2.0);
          QMouseEvent blackPress(QEvent::MouseButtonPress, right, Qt::LeftButton,
                                 Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(mixerWheel, &blackPress);
          check(str(object(state.value("mixer")), "current") == "#00000080" &&
                    qAbs(mixerSpectrum->hue() - mixerWheel->hue()) < .01,
                "Wheel preserves black value and alpha while updating gradient hue");
          QMouseEvent blackRelease(QEvent::MouseButtonRelease, right, Qt::LeftButton,
                                   Qt::NoButton, Qt::NoModifier);
          QApplication::sendEvent(mixerWheel, &blackRelease);
        }
        send({{"op", "mixer_select"}, {"color", "#04050642"}});
        auto selectedHsv = array(object(state.value("mixer")), "hsv");
        check(selectedHsv.size() == 3 && mixerWheel && mixerSpectrum &&
                  qAbs(mixerWheel->hue() - selectedHsv[0].toDouble()) < .01 &&
                  qAbs(mixerSpectrum->saturation() - selectedHsv[1].toDouble()) < .01,
              "Color selection updates both mixer selectors");
        QAction *labels = nullptr, *historyToggle = nullptr;
        for (auto action : findChildren<QAction *>()) {
          if (action->text() == "Show labels") labels = action;
          if (action->text() == "History") historyToggle = action;
        }
        check(labels && historyToggle, "Toolbar actions available");
        if (labels) {
          labels->setChecked(false);
          auto actions = tabs->widget(0)->findChild<QToolBar *>();
          check(actions && actions->toolButtonStyle() == Qt::ToolButtonIconOnly,
                "Toolbar labels can be hidden");
          labels->setChecked(true);
        }
        if (historyToggle) {
          historyToggle->setChecked(false);
          check(!historyScroll->isVisible(), "History collapses");
          historyToggle->setChecked(true);
          check(historyScroll->isVisible(), "History expands");
        }
        selectFormat("HEX");
        QSize mixerSize = size();
        resize(900, 700);
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        auto mixerPage = tabs->widget(0);
        auto mixerScroll = mixerPage->findChild<QScrollArea *>();
        auto boundsInMixer = [mixerScroll](QWidget *widget) {
          return QRect(widget->mapTo(mixerScroll->viewport(), QPoint()), widget->size());
        };
        auto inMixerViewport = [&](QWidget *widget) {
          return mixerScroll->viewport()->rect().contains(boundsInMixer(widget));
        };
        auto hueInput = findHsv("Hue");
        check(mixerSpectrum->width() == mixerSpectrum->height() &&
                  inMixerViewport(mixerSpectrum) && inMixerViewport(mixerWheel) &&
                  hueInput && inMixerViewport(hueInput) &&
                  boundsInMixer(mixerSpectrum).right() < boundsInMixer(mixerWheel).left() &&
                  boundsInMixer(mixerWheel).right() < boundsInMixer(hueInput).left(),
              "Wide Mixer shows a square spectrum, wheel, and inputs in one row");
        auto wideActions = mixerPage->findChild<QToolBar *>("mixerActions");
        check(wideActions && wideActions->parentWidget() == mixerPage &&
                  wideActions->isVisible() &&
                  !mixerScroll->widget()->findChild<QToolBar *>("mixerActions"),
              "Wide Mixer pins its toolbar above the scroll");
        for (auto b : mixerPage->findChildren<QAbstractButton *>())
          if (b->accessibleName() == "Copy" || b->accessibleName() == "Add to palette") {
            check(b->isVisible() &&
                      mixerPage->rect().contains(
                          QRect(b->mapTo(mixerPage, QPoint()), b->size())),
                  b->accessibleName() + " visible in initial Mixer");
          }
        auto histHdr = historyPanel->findChild<QWidget *>("historyHeader");
        auto curRow = pinnedCurrent->findChild<QWidget *>("currentColorRow");
        check(histHdr && curRow &&
                  histHdr->mapTo(splitter, QPoint(0, histHdr->height())).y() ==
                      curRow->mapTo(splitter, QPoint(0, curRow->height())).y(),
              "History and CurrentColorBanner header hairlines align flush");
        shot("mixer");
        resize(540, 360);
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        check(mixerSpectrum->width() == mixerSpectrum->height() &&
                  inMixerViewport(mixerSpectrum) && inMixerViewport(mixerWheel) &&
                  hueInput && boundsInMixer(hueInput).top() > boundsInMixer(mixerSpectrum).bottom() &&
                  mixerScroll->horizontalScrollBar()->maximum() == 0,
              "Compact Mixer shows both selectors and stacks inputs in a vertical scroll");
        auto actions = mixerPage->findChild<QToolBar *>("mixerActions");
        check(actions->parentWidget() == mixerPage && actions->isVisible() &&
                  !mixerScroll->widget()->findChild<QToolBar *>("mixerActions"),
              "Compact Mixer pins its only toolbar above the scroll");
        for (const auto &name :
             {QString("Copy"), QString("Add to palette"), QString("Undo"), QString("Redo")}) {
          auto matches = mixerPage->findChildren<QAbstractButton *>();
          matches.removeIf([&](QAbstractButton *b) { return b->accessibleName() != name; });
          check(matches.size() == 1 && matches.first()->isVisible() &&
                    mixerPage->rect().contains(QRect(matches.first()->mapTo(mixerPage, QPoint()),
                                                      matches.first()->size())),
                name + " is directly visible in the initial compact Mixer");
        }
        auto overflow = actions->findChild<QToolButton *>("qt_toolbar_ext_button");
        check(!overflow || !overflow->isVisible(),
              "Compact mixer toolbar fits all primary actions without overflow");
        auto emptyDrop = pinnedScroll->findChild<QLabel *>();
        check(emptyDrop && emptyDrop->acceptDrops() && emptyDrop->isVisible() &&
                  pinnedScroll->height() <= fontMetrics().height() + 8,
              "Compact Mixer retains a one-line empty palette drop target");
        auto rgbaCompact = array(object(state.value("mixer")), "rgba");
        check(mixerReadout && mixerReadout->isVisible() && inMixerViewport(mixerReadout) &&
                  mixerReadout->text() ==
                      mixerHsvReadout(210.0, 100.0 / 3, 600.0 / 255,
                                      rgbaCompact.size() == 4 ? rgbaCompact[3].toInt() : 255),
              "Compact Mixer displays current HSV and alpha values without scrolling");
        shot("mixer-small");
        auto mixSection = mixerScroll->widget()->findChild<QWidget *>("mixerMixHeader");
        QAbstractButton *addSourceBtn = nullptr, *mixBtn = nullptr;
        if (mixSection) {
          for (auto b : mixSection->findChildren<QAbstractButton *>()) {
            if (b->accessibleName() == "Add source") addSourceBtn = b;
            if (b->accessibleName() == "Mix") mixBtn = b;
          }
        }
        check(addSourceBtn && addSourceBtn->isEnabled() &&
                  mixBtn && mixBtn->isEnabled(),
              "Add source and Mix are enabled inside the Color mixing section");
        for (auto field : mixerScroll->findChildren<QLineEdit *>())
          if (field->accessibleName() == "Color value") {
            mixerScroll->verticalScrollBar()->setValue(
                field->mapTo(mixerScroll->widget(), QPoint()).y());
            settleLayout();
            if (mixSection && !inMixerViewport(mixSection))
              mixerScroll->ensureWidgetVisible(mixSection, 0, 8);
            check(addSourceBtn && addSourceBtn->isVisible() && inMixerViewport(addSourceBtn) &&
                      mixBtn && mixBtn->isVisible() && inMixerViewport(mixBtn),
                  "Add source and Mix are visible in compact Color mixing section");
            shot("mixer-small-format");
            break;
          }
        click(mixerPage, "Copy");
        check(QApplication::clipboard()->text() == "#040506", "Pinned Copy copies the Mixer output");
        const QString beforeMix = str(object(state.value("mixer")), "current");
        // Bug 4 regression: focusing and leaving an unchanged mixing source
        // field must not send mixer_source or overwrite the current color.
        for (auto field : tabs->widget(0)->findChildren<QLineEdit *>())
          if (field->accessibleName() == "Source color") {
            QMetaObject::invokeMethod(field, "editingFinished");
            break;
          }
        for (auto spinBox : tabs->widget(0)->findChildren<QDoubleSpinBox *>())
          if (spinBox->accessibleName() == "Source weight") {
            QMetaObject::invokeMethod(spinBox, "editingFinished");
            break;
          }
        check(str(object(state.value("mixer")), "current") == beforeMix,
              "Unchanged mixing source focus leaves current color untouched");
        click(tabs->widget(0), "Mix");
        check(str(object(state.value("mixer")), "current") != beforeMix,
              "Mix computes a new current color from sources");
        click(tabs->widget(0), "Undo");
        check(str(object(state.value("mixer")), "current") == beforeMix,
              "Undo restores current color after Mix");
        const int sourcesBefore = array(object(state.value("mixer")), "sources").size();
        click(tabs->widget(0), "Add source");
        check(array(object(state.value("mixer")), "sources").size() == sourcesBefore + 1,
              "Add source appends a mixing source");
        click(tabs->widget(0), "Undo");
        check(array(object(state.value("mixer")), "sources").size() == sourcesBefore &&
                  str(object(state.value("mixer")), "current") == beforeMix,
              "Undo restores sources after Add source");
        mixerPage = tabs->widget(0);
        if (emptyDrop) {
          QMimeData mime;
          mime.setText("#04050642");
          QDragEnterEvent enter(QPoint(1, 1), Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(emptyDrop, &enter);
          QDropEvent drop(QPointF(1, 1), Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(emptyDrop, &drop);
          check(enter.isAccepted() && drop.isAccepted() &&
                    array(object(state.value("palette")), "swatches").size() == 1,
                "Compact empty palette accepts a color drop");
          send({{"op", "palette_undo"}});
        }
        click(mixerPage, "Add to palette");
        check(!array(object(state.value("palette")), "swatches").isEmpty(),
              "Pinned Add to palette adds the selected color");
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        auto populated = pinnedScroll->findChildren<QPushButton *>();
        check(!populated.isEmpty() && populated.first()->isVisible(),
              "Compact Mixer still displays populated palette swatches");
        resize(mixerSize);
        auto activation = state;
        activation["show_palette"] = true;
        updateState(activation);
        check(tabs->currentIndex() == 1, "Palette activation selects Palette");
        tabs->setCurrentIndex(1);
        click(tabs->widget(1), "New draft");
        tabs->setCurrentIndex(0);
        click(tabs->widget(0), "Add to palette");
        tabs->setCurrentIndex(1);
        check(!array(object(state.value("palette")), "swatches").isEmpty(),
              "Add swatch");
        send({{"op", "mixer_select"}, {"color", "#AABBCCFF"}});
        send({{"op", "palette_add_current"}});
        QList<Swatch *> swatches;
        for (auto b : pinnedScroll->findChildren<QPushButton *>())
          if (auto swatch = dynamic_cast<Swatch *>(b)) swatches.append(swatch);
        check(swatches.size() == 2, "Two palette swatches available");
        if (swatches.size() == 2) {
          QMimeData mime;
          mime.setText("#04050642");
          mime.setData("application/x-swatch-board-index", "0");
          QDragEnterEvent enter(QPoint(1, 1), Qt::CopyAction, &mime,
                                Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(swatches[1], &enter);
          QDropEvent drop(QPointF(1, 1), Qt::CopyAction, &mime,
                          Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(swatches[1], &drop);
          auto reordered = array(object(state.value("palette")), "swatches");
          check(reordered.size() == 2 &&
                    str(reordered[0].toObject(), "color") == "#AABBCCFF" &&
                    str(reordered[1].toObject(), "color") == "#04050642",
                "Palette drag reorders colors");
        }
        for (auto b : pinnedScroll->findChildren<QPushButton *>())
          if (dynamic_cast<Swatch *>(b) && b->text() == "#04050642") {
            b->click();
            break;
          }
        check(str(object(state.value("mixer")), "current") == "#04050642",
              "Palette swatch selects mixer color");
        tabs->setCurrentIndex(1);
        activateWindow();
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        tabs->setFocus();
        auto paletteBeforeUndo = array(object(state.value("palette")), "swatches");
        auto mixerBeforePaletteUndo = str(object(state.value("mixer")), "current");
        key(tabs, Qt::Key_Z, Qt::ControlModifier);
        check(array(object(state.value("palette")), "swatches") != paletteBeforeUndo,
              "Palette undo keyboard shortcut");
        check(str(object(state.value("mixer")), "current") == mixerBeforePaletteUndo,
              "Palette undo leaves the current mixer color unchanged");
        tabs->setFocus();
        key(tabs, Qt::Key_Z, Qt::ControlModifier | Qt::ShiftModifier);
        check(array(object(state.value("palette")), "swatches") == paletteBeforeUndo,
              "Palette redo keyboard shortcut");
        auto palActions = tabs->widget(1)->findChild<QToolBar *>("paletteActions");
        check(palActions && palActions->parentWidget() == tabs->widget(1) &&
                  palActions->isVisible() && histHdr &&
                  histHdr->mapTo(splitter, QPoint(0, histHdr->height())).y() ==
                      palActions->mapTo(splitter, QPoint(0, palActions->height())).y(),
              "History and Palette toolbar hairlines align flush");
        auto palPreview = dynamic_cast<PreviewStrip *>(
            tabs->widget(1)->findChild<QWidget *>("palettePreviewStrip"));
        check(palPreview && palPreview->colorCount() > 0,
              "Palette preview populates immediately");
        // Bug 14 regression: pinned palette shelf never spawns a vertical scrollbar.
        if (auto *shelfScroll =
                pinnedScroll->findChild<QScrollArea *>("pinnedShelfScroll")) {
          check(shelfScroll->verticalScrollBarPolicy() ==
                        Qt::ScrollBarAlwaysOff &&
                    shelfScroll->verticalScrollBar()->maximum() == 0,
                "Pinned palette shelf never spawns a vertical scrollbar");
        }
        if (array(object(state.value("palette")), "saved").isEmpty()) {
          for (auto b : tabs->widget(1)->findChildren<QAbstractButton *>())
            if (b->accessibleName() == "Open" || b->accessibleName() == "Delete")
              check(!b->isEnabled(), b->accessibleName() + " disabled when library is empty");
        }
        shot("palette");
        const int normalShelfHeight = pinnedScroll->height();
        resize(540, 360);
        shot("palette-small-library");
        check(pinnedScroll->height() < normalShelfHeight,
              "Palette shelf shrinks at compact size");
        auto generator = tabs->widget(1)->findChild<QWidget *>("paletteGenerator");
        auto generate = generator ? generator->findChild<QPushButton *>(QString(), Qt::FindChildrenRecursively) : nullptr;
        if (generator) {
          for (auto b : generator->findChildren<QPushButton *>())
            if (b->text() == "Generate") generate = b;
        }
        check(generate && generate->text() == "Generate", "Generate control available");
        if (generate) {
          for (auto parent = generate->parentWidget(); parent; parent = parent->parentWidget())
            if (auto scroll = qobject_cast<QScrollArea *>(parent)) {
              scroll->ensureWidgetVisible(generate);
              break;
            }
          GeneratorWheel *wheel = nullptr;
          for (auto widget : generator->findChildren<QWidget *>())
            if ((wheel = dynamic_cast<GeneratorWheel *>(widget))) break;
          auto paletteScroll = tabs->widget(1)->findChild<QScrollArea *>();
          if (wheel && paletteScroll) paletteScroll->ensureWidgetVisible(wheel, 0, 0);
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
          auto viewport = paletteScroll ? paletteScroll->viewport() : nullptr;
          check(wheel && viewport && viewport->rect().contains(QRect(wheel->mapTo(viewport, QPoint()), wheel->size())) &&
                    viewport->rect().contains(QRect(generate->mapTo(viewport, QPoint()), generate->size())),
                "Compact palette shows complete wheel and Generate together");
          shot("palette-small");
          generate->click();
          check(array(object(state.value("palette")), "swatches").size() == 5,
                "Generate palette at compact size");
        }
        auto captureSavedGenerations = [&](const QString &name) {
          settleLayout();
          auto scroll = tabs->widget(1)->findChild<QScrollArea *>();
          auto row = scroll ? scroll->findChild<QWidget *>("savedGenerationRow") : nullptr;
          auto label = scroll ? scroll->findChild<QLabel *>("savedGenerationLabel") : nullptr;
          check(row && label, "Saved generation controls available");
          if (!row || !label)
            return;
          const int top = label->mapTo(scroll->widget(), QPoint()).y();
          scroll->verticalScrollBar()->setValue(top - 32);
          shot(name);
          auto viewport = scroll->viewport();
          const QRect rowBounds(row->mapTo(viewport, QPoint()), row->size());
          const QRect labelBounds(label->mapTo(viewport, QPoint()), label->size());
          check(viewport->rect().contains(rowBounds.united(labelBounds)),
                "Full saved generation row visible in " + name);
          auto selector = row->findChild<QComboBox *>();
          check(selector && !selector->currentText().isEmpty(),
                "Saved generation description available in " + name);
          if (selector) {
            QStyleOptionComboBox option;
            option.initFrom(selector);
            const auto textBounds = selector->style()->subControlRect(
                QStyle::CC_ComboBox, &option, QStyle::SC_ComboBoxEditField,
                selector);
            check(selector->fontMetrics().horizontalAdvance(selector->currentText()) <=
                      textBounds.width(),
                  "Saved generation description fits in " + name);
          }
        };
        captureSavedGenerations("palette-small-generations");
        resize(900, 700);
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        captureSavedGenerations("palette-generations");
        click(tabs->widget(1), "New draft");
        click(tabs->widget(1), "Load generation");
        check(array(object(state.value("palette")), "swatches").size() == 5,
              "Load saved generation");
        auto edits = tabs->widget(1)->findChildren<QLineEdit *>();
        for (auto e : edits)
          if (e->accessibleName() == "Palette name") {
            // Bug 1 regression: editing Palette name without pressing Enter
            // and clicking toolbar Save must commit the name without crashing.
            e->setFocus(Qt::OtherFocusReason);
            e->setText("Qt smoke palette");
            e->setModified(true);
            break;
          }
        click(tabs->widget(1), "Save");
        check(str(object(state.value("palette")), "name") == "Qt smoke palette",
              "Toolbar Save commits focused Palette name edit without crashing");
        auto id = str(object(state.value("palette")), "id");
        check(!id.isEmpty(), "Save palette");
        click(tabs->widget(1), "Open");
        check(str(object(state.value("palette")), "id") == id,
              "Reopen palette");
        tabs->setCurrentIndex(2);
        // Bug 3 regression: empty Theme page disables Save, Use active palette,
        // and Export controls while keeping Project enabled.
        for (auto b : tabs->widget(2)->findChildren<QAbstractButton *>()) {
          const QString an = b->accessibleName();
          if (an == "Save" || an == "Use active palette" || an == "Export" ||
              an == "Choose directory")
            check(!b->isEnabled(), an + " disabled without theme project");
          if (an == "Project")
            check(b->isEnabled(), "Project enabled without theme project");
        }
        for (auto e : tabs->widget(2)->findChildren<QLineEdit *>())
          if (e->accessibleName() == "Export directory")
            check(!e->isEnabled(), "Export directory disabled without theme project");
        for (auto cb : tabs->widget(2)->findChildren<QCheckBox *>())
          if (cb->text().contains("Overwrite"))
            check(!cb->isEnabled(), "Overwrite checkbox disabled without theme project");
        check(!array(object(state.value("theme")), "plugins").isEmpty(),
              "Theme plugin available");
        if (!array(object(state.value("theme")), "plugins").isEmpty()) {
          click(tabs->widget(2), "Project");
          auto dialog = tabs->widget(2)->findChild<QDialog *>("themeProjectDialog");
          check(dialog && dialog->isVisible(), "Theme project opens without a project");
          click(tabs->widget(2), "New theme");
          check(!array(object(state.value("theme")), "roles").isEmpty(), "New theme from project dialog");
          dialog = tabs->widget(2)->findChild<QDialog *>("themeProjectDialog");
          check(dialog && dialog->isVisible(), "Theme project remains open after new theme");
          if (dialog) {
            auto name = dialog->findChild<QLineEdit *>();
            check(name && name->accessibleName() == "Theme name", "Theme project name available");
            if (name) {
              // Bug 2 regression: editing Theme name and clicking Close must
              // commit the name and close on the first click without reopening.
              name->setFocus(Qt::OtherFocusReason);
              name->setText("Smoke theme");
              name->setModified(true);
              QPushButton *closeBtn = nullptr;
              if (auto *box = dialog->findChild<QDialogButtonBox *>())
                closeBtn = box->button(QDialogButtonBox::Close);
              check(closeBtn != nullptr, "Theme project Close button available");
              if (closeBtn) {
                const QPoint c = closeBtn->rect().center();
                // Simulate real mouse-click focus transfer from Theme name to
                // Close before press/release.
                QFocusEvent focusOut(QEvent::FocusOut, Qt::MouseFocusReason);
                QApplication::sendEvent(name, &focusOut);
                QMouseEvent press(QEvent::MouseButtonPress, c,
                                  closeBtn->mapToGlobal(c), Qt::LeftButton,
                                  Qt::LeftButton, Qt::NoModifier);
                QApplication::sendEvent(closeBtn, &press);
                QMouseEvent release(QEvent::MouseButtonRelease, c,
                                    closeBtn->mapToGlobal(c), Qt::LeftButton,
                                    Qt::NoButton, Qt::NoModifier);
                QApplication::sendEvent(closeBtn, &release);
                QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
                auto *afterDialog =
                    tabs->widget(2)->findChild<QDialog *>("themeProjectDialog");
                check(str(object(object(state.value("theme")).value("project")), "name") == "Smoke theme",
                      "Theme project name updated");
                check(afterDialog == dialog && !afterDialog->isVisible(),
                      "Theme project closes on first click after editing name");
              }
            }
          }
          click(tabs->widget(2), "Project");
          dialog = tabs->widget(2)->findChild<QDialog *>("themeProjectDialog");
          if (dialog) {
            QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
            check(dialog->grab().save(dir + "/theme-project.png"), "Screenshot theme project");
            dialog->reject();
            check(!dialog->isVisible(), "Theme project closes");
          }
          // Bug 16 regression: Palette swatch link combobox includes explicit Unlinked entry.
          bool foundLinkCombo = false;
          for (auto combo : tabs->widget(2)->findChildren<QComboBox *>()) {
            if (combo->accessibleName() == "Palette swatch link") {
              foundLinkCombo = true;
              check(combo->count() > 0 && combo->itemText(0) == "Unlinked" &&
                        combo->itemData(0).toString().isEmpty(),
                    "Palette link combobox has explicit Unlinked item");
              break;
            }
          }
          check(foundLinkCombo, "Palette link combobox available");
          check(!pinnedCurrent->isVisible(), "Mixer current color hidden on Theme");
          for (auto b : boardContent->parentWidget()->findChildren<QPushButton *>())
            if (b->accessibleName().startsWith("Current color"))
              check(!b->isVisible(), "No stale mixer color on Theme");
          auto roles = array(object(state.value("theme")), "roles");
          auto previewBefore = object(state.value("theme")).value("preview");
          auto fields = tabs->widget(2)->findChildren<QLineEdit *>();
          bool roleFieldFound = false;
          for (auto field : fields) {
            if (field->accessibleName() != "Role color")
              continue;
            roleFieldFound = true;
            QMimeData mime;
            mime.setText("#112233FF");
            QDragEnterEvent enter(QPoint(1, 1), Qt::CopyAction, &mime,
                                  Qt::LeftButton, Qt::NoModifier);
            QApplication::sendEvent(field, &enter);
            QDropEvent drop(QPointF(1, 1), Qt::CopyAction, &mime,
                            Qt::LeftButton, Qt::NoModifier);
            QApplication::sendEvent(field, &drop);
            auto updated = array(object(state.value("theme")), "roles");
            check(!roles.isEmpty() && !updated.isEmpty() &&
                      str(updated[0].toObject(), "color") == "#112233FF",
                  "Theme color drop replaces value");
            check(object(state.value("theme")).value("preview") != previewBefore,
                  "Theme preview updates after color drop");
            break;
          }
          check(roleFieldFound, "Theme role color field available");
          activateWindow();
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
          tabs->setFocus();
          auto beforeThemeUndo = array(object(state.value("theme")), "roles");
          key(tabs, Qt::Key_Z, Qt::ControlModifier);
          check(array(object(state.value("theme")), "roles") == roles, "Theme undo keyboard shortcut");
          tabs->setFocus();
          key(tabs, Qt::Key_Z, Qt::ControlModifier | Qt::ShiftModifier);
          check(array(object(state.value("theme")), "roles") == beforeThemeUndo,
                "Theme redo keyboard shortcut");
          Scene *scenePreview = nullptr;
          for (auto widget : tabs->widget(2)->findChildren<QWidget *>())
            if ((scenePreview = dynamic_cast<Scene *>(widget))) break;
          check(scenePreview && scenePreview->isVisible(), "Theme scene preview available");
          if (scenePreview && scenePreview->isVisible()) {
            QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
            // The example terminal's center falls on its background.
            const QPoint position = scenePreview->targetRect().center().toPoint();
            QMimeData mime;
            mime.setText("#142638FF");
            QDragEnterEvent enter(position, Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
            QApplication::sendEvent(scenePreview, &enter);
            QDropEvent drop(QPointF(position), Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
            QApplication::sendEvent(scenePreview, &drop);
            const auto updated = array(object(state.value("theme")), "roles");
            check(drop.isAccepted() && !updated.isEmpty() &&
                      str(updated[0].toObject(), "color") == "#142638FF",
                  "Theme scene drop updates the background role");
          }
          // Bug 1 regression: editing Role color without pressing Enter and
          // clicking Theme toolbar Save must commit the color without crashing.
          for (auto field : tabs->widget(2)->findChildren<QLineEdit *>()) {
            if (field->accessibleName() == "Role color") {
              field->setFocus(Qt::OtherFocusReason);
              field->setText("#224466FF");
              field->setModified(true);
              break;
            }
          }
          click(tabs->widget(2), "Save");
          check(str(array(object(state.value("theme")), "roles")[0].toObject(),
                    "color") == "#224466FF",
                "Theme toolbar Save commits focused Role color without crashing");
          click(tabs->widget(2), "Project");
          auto projectDialog = tabs->widget(2)->findChild<QDialog *>("themeProjectDialog");
          QComboBox *savedThemes = nullptr;
          if (projectDialog)
            for (auto combo : projectDialog->findChildren<QComboBox *>())
              if (combo->accessibleName() == "Saved themes") savedThemes = combo;
          check(savedThemes && savedThemes->count() > 0, "Saved theme available in project dialog");
          if (savedThemes && savedThemes->count() > 0) {
            savedThemes->setCurrentIndex(0);
            click(tabs->widget(2), "Open theme");
            check(str(object(object(state.value("theme")).value("project")), "name") == "Smoke theme",
                  "Open saved theme from project dialog");
          }
          projectDialog = tabs->widget(2)->findChild<QDialog *>("themeProjectDialog");
          if (projectDialog) projectDialog->reject();
        }
        auto themePanes = [&] { return tabs->widget(2)->findChild<QSplitter *>("themePanes"); };
        auto widePanes = themePanes();
        auto wideScene = static_cast<Scene *>(tabs->widget(2)->findChild<QWidget *>("themeScene"));
        QList<int> adjustedSizes;
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        check(widePanes && widePanes->orientation() == Qt::Horizontal,
              "Theme preview and inspector sit beside each other at normal size");
        if (wideScene) {
          check(wideScene->targetRect().width() >= 350,
                "Theme scene has room to render at normal size");
          const auto point = wideScene->targetRect().center().toPoint();
          QMouseEvent press(QEvent::MouseButtonPress, point, wideScene->mapToGlobal(point),
                            Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(wideScene, &press);
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
          auto selected = tabs->widget(2)->findChild<QWidget *>("selectedThemeRole");
          auto inspector = tabs->widget(2)->findChild<QScrollArea *>("themeInspector");
          check(selected && inspector && str(object(state.value("theme")), "selected") == "background" &&
                    inspector->viewport()->rect().contains(
                        QRect(selected->mapTo(inspector->viewport(), QPoint()), selected->size())),
                "Clicking the scene shows its selected role in the inspector");
          check(inspector && inspector->horizontalScrollBar()->maximum() == 0,
                "Theme inspector fits at normal size without horizontal scrolling");
        }
        shot("theme");
        widePanes = themePanes();
        if (widePanes && widePanes->orientation() == Qt::Horizontal) {
          auto handle = widePanes->handle(1);
          const auto before = widePanes->sizes();
          const auto point = handle->rect().center();
          const auto global = handle->mapToGlobal(point);
          const auto target = global - QPoint(35, 0);
          QMouseEvent press(QEvent::MouseButtonPress, point, global,
                            Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(handle, &press);
          QMouseEvent move(QEvent::MouseMove, handle->mapFromGlobal(target), target,
                           Qt::NoButton, Qt::LeftButton, Qt::NoModifier);
          QApplication::sendEvent(handle, &move);
          QMouseEvent release(QEvent::MouseButtonRelease, handle->mapFromGlobal(target), target,
                              Qt::LeftButton, Qt::NoButton, Qt::NoModifier);
          QApplication::sendEvent(handle, &release);
          adjustedSizes = widePanes->sizes();
          check(adjustedSizes != before &&
                    settings.value("board/themeSplitter").toList() ==
                        QVariantList{adjustedSizes[0], adjustedSizes[1]},
                "Dragging the Theme divider saves its pane sizes");
        }
        resize(540, 360);
        shot("theme-small");
        click(tabs->widget(2), "Project");
        if (auto dialog = tabs->widget(2)->findChild<QDialog *>("themeProjectDialog")) {
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
          check(dialog->isVisible() && dialog->size().width() <= width() && dialog->size().height() <= height(),
                "Theme project fits at small size");
          check(dialog->grab().save(dir + "/theme-project-small.png"), "Screenshot small theme project");
          dialog->reject();
        } else check(false, "Theme project dialog available at small size");
        check(size() == QSize(540, 360), "Board reaches 540x360");
        auto visibleInWindow = [this](QWidget *widget) {
          if (!widget || !widget->isVisible()) return false;
          QRect bounds(widget->mapTo(this, QPoint()), widget->size());
          return bounds.intersected(rect()).height() >= 20 &&
                 bounds.intersected(rect()).width() >= 20;
        };
        Scene *scene = nullptr;
        for (auto widget : tabs->widget(2)->findChildren<QWidget *>())
          if ((scene = dynamic_cast<Scene *>(widget))) break;
        auto themeScrolls = tabs->widget(2)->findChildren<QScrollArea *>();
        check(!themeScrolls.isEmpty() && visibleInWindow(themeScrolls.first()),
              "Theme controls visible at small size");
        auto previewToggle = tabs->widget(2)->findChild<QToolButton *>("themePreviewToggle");
        check(scene && !scene->isVisible() && previewToggle && !previewToggle->isChecked(),
              "Theme preview collapses at small size");
        if (previewToggle && scene) {
          previewToggle->click();
          check(scene->isVisible(), "Theme preview can be expanded");
          auto previewContainer = tabs->widget(2)->findChild<QWidget *>("themePreviewContainer");
          auto sceneSelector = previewContainer ? previewContainer->findChild<QComboBox *>() : nullptr;
          check(sceneSelector && sceneSelector->isVisible() &&
                    sceneSelector->accessibleName() == "Preview scene",
                "Theme scene selector stays with the expanded preview");
          shot("theme-small-preview");
          auto inspector = tabs->widget(2)->findChild<QScrollArea *>("themeInspector");
          auto firstRole = tabs->widget(2)->findChild<QWidget *>("selectedThemeRole");
          auto roleValue = firstRole
              ? firstRole->findChild<QLineEdit *>(QString(), Qt::FindDirectChildrenOnly)
              : nullptr;
          check(inspector && roleValue &&
                    inspector->viewport()->rect().contains(
                        QRect(roleValue->mapTo(inspector->viewport(), QPoint()), roleValue->size())),
                "First role editor remains visible with compact preview expanded");
          previewToggle->click();
          check(!scene->isVisible(), "Theme preview can be collapsed");
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        }
        if (!themeScrolls.isEmpty()) {
          auto scroll = themeScrolls.first();
          auto insideViewport = [scroll](QWidget *widget) {
            if (!widget || !widget->isVisible()) return false;
            QRect bounds(widget->mapTo(scroll->viewport(), QPoint()), widget->size());
            return scroll->viewport()->rect().contains(bounds);
          };
          QLineEdit *role = nullptr;
          for (auto field : tabs->widget(2)->findChildren<QLineEdit *>())
            if (field->accessibleName() == "Role color") { role = field; break; }
          QAbstractButton *save = nullptr, *exportButton = nullptr;
          for (auto b : tabs->widget(2)->findChildren<QAbstractButton *>()) {
            if (b->accessibleName() == "Save") save = b;
            if (b->accessibleName() == "Export") exportButton = b;
          }
          check(insideViewport(role) && visibleInWindow(save),
                "Theme role and Save visible at small size");
          if (exportButton && save) {
            activateWindow();
            QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
            save->setFocus(Qt::TabFocusReason);
            for (int i = 0; i < 100 && !exportButton->hasFocus(); ++i) {
              auto focused = QApplication::focusWidget();
              if (!focused) break;
              QKeyEvent tab(QEvent::KeyPress, Qt::Key_Tab, Qt::NoModifier);
              QApplication::sendEvent(focused, &tab);
              QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
            }
            check(insideViewport(exportButton) && exportButton->hasFocus(),
                  "Theme export reachable by keyboard focus");
            shot("theme-small-bottom");
          } else check(false, "Theme export available");
        }
        check(visibleInWindow(pinnedScroll), "Palette visible at small size");
        for (int page : {0, 1}) {
          tabs->setCurrentIndex(page);
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
          check(visibleInWindow(tabs->widget(page)),
                page == 0 ? "Mixer visible at small size" : "Palette page visible at small size");
          check(visibleInWindow(pinnedScroll), "Shared palette remains visible at small size");
        }
        if (!adjustedSizes.isEmpty()) {
          resize(900, 700);
          tabs->setCurrentIndex(2);
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
          auto restoredPanes = themePanes();
          check(restoredPanes && restoredPanes->orientation() == Qt::Horizontal &&
                    qAbs(restoredPanes->sizes()[0] - adjustedSizes[0]) <= 2,
                "Theme divider position survives compact mode and page refreshes");
        }
      } else if (mode == "config") {
        auto path = qEnvironmentVariable("XDG_CONFIG_HOME") +
                    "/swatch-board/config.toml";
        QFile beforeFile(path);
        QByteArray before;
        if (beforeFile.open(QIODevice::ReadOnly))
          before = beforeFile.readAll();
        edit(this, "Stored history limit", "0");
        click(this, "Save settings");
        check(!str(state, "message").isEmpty() ||
                  !str(state, "load_error").isEmpty(),
              "Settings validation");
        QFile afterFile(path);
        QByteArray after;
        if (afterFile.open(QIODevice::ReadOnly))
          after = afterFile.readAll();
        check(before == after, "Invalid settings unchanged on disk");
        check(configSave && !configSave->isEnabled(),
              "Save disabled on validation error");
        check(limitErrorBox && limitErrorBox->isVisible(),
              "Limit validation banner visible");
        shot("settings");
        check(configScroll &&
                  configScroll->verticalScrollBar()->maximum() == 0,
              "Settings fits without vertical scrolling at default size");
        resize(540, 360);
        shot("settings-small");
        check(configScroll &&
                  configScroll->horizontalScrollBar()->maximum() == 0,
              "Compact settings has no horizontal overflow");
        edit(this, "Stored history limit", "200");
        check(configSave && configSave->isEnabled(),
              "Save re-enabled after editing invalid limit");
        check(limitErrorBox && !limitErrorBox->isVisible(),
              "Limit validation banner hides after edit");
        // Bug 6 regression: re-submitting invalid limit after editing must
        // re-show the validation error instead of being silently dropped.
        edit(this, "Stored history limit", "0");
        click(this, "Save settings");
        check(configSave && !configSave->isEnabled() && limitErrorBox &&
                  limitErrorBox->isVisible(),
              "Re-submitting invalid limit re-shows validation error");
        // Bug 18 regression: HistoryLimitSpinBox overflow stepping clamps to i64::MAX.
        if (configLimit) {
          configLimit->lineEdit()->setText("99999999999999999999");
          configLimit->stepUp();
          check(configLimit->lineEdit()->text() != "1",
                "Overflow stepUp does not wrap down to 1");
          configLimit->stepDown();
          check(configLimit->lineEdit()->text() == "9223372036854775807",
                "Overflow stepDown clamps to i64::MAX");
        }
        // Bug 19 regression: long KDE fallback status wraps without horizontal overflow.
        auto longStatusState = state;
        longStatusState["shortcut_status"] =
            "GlobalShortcuts unavailable; configure KDE shortcut calling "
            "org.apartment38.SwatchBoard1.ShowHistory";
        updateState(longStatusState, true);
        settleLayout();
        check(configScroll &&
                  configScroll->horizontalScrollBar()->maximum() == 0,
              "Long KDE fallback shortcut status has no horizontal overflow");
        // Bug 20 regression: load_error disables Rebind shortcuts.
        auto loadErrState = state;
        loadErrState["load_error"] = "Failed to parse config.toml";
        updateState(loadErrState, true);
        check(configRebind && !configRebind->isEnabled(),
              "Rebind shortcuts disabled during load_error");
      } else {
        check(state.contains("entries"), "History entries");
        shot("history");
        // Bug 8 regression: copy error banner renders outside QScrollArea.
        auto errState = state;
        errState["error"] = "Clipboard copy failed: Wayland compositor unavailable";
        updateState(errState);
        settleLayout();
        auto *errBanner = findChild<QWidget *>("historyErrorBanner");
        check(errBanner && errBanner->isVisible() &&
                  (!historyScroll || !historyScroll->isAncestorOf(errBanner)),
              "History copy error banner pinned visibly outside scroll area");
        shot("history-error");
      }
      QFile file(dir + "/" + mode + "-result.json");
      if (file.open(QIODevice::WriteOnly))
        file.write(QJsonDocument(QJsonObject{{"failures", failures}}).toJson());
      else
        failures.append("Cannot write result");
      for (auto failure : failures)
        qCritical().noquote() << failure.toString();
      QCoreApplication::exit(failures.isEmpty() ? 0 : 1);
    });
  }
  Frontend(QString m, Callback cb, void *ctx)
      : callback(cb), context(ctx), mode(m) {
    setWindowTitle(m == "board"    ? "Swatch Board"
                   : m == "config" ? "Swatch Board Settings"
                                   : "Swatch Board History");
    setObjectName("swatch_" + m);
    root = new QVBoxLayout(this);
    if (m == "board" || m == "config" || m == "history") {
      root->setContentsMargins(0, 0, 0, 0);
      root->setSpacing(0);
    }
    if (m == "history") {
      setWindowFlags(Qt::Popup);
      setAttribute(Qt::WA_NoMouseReplay);
    } else {
      resize(m == "config" ? QSize(580, 656) : QSize(900, 700));
      restoreGeometry(settings.value(m + "/geometry").toByteArray());
    }
    auto initial = request({{"op", "init"}});
    if (initial.isObject())
      updateState(initial.toObject());
    connect(&timer, &QTimer::timeout, this, [this] {
      auto r = request({{"op", "poll"}});
      if (r.isObject())
        updateState(r.toObject());
    });
    if (m != "history")
      timer.start(100);
  }
  ~Frontend() override {
    if (mode != "history")
      settings.setValue(mode + "/geometry", saveGeometry());
    if (splitter && verticalSplitter) {
      settings.setValue("board/horizontalSplitter", splitter->saveState());
      settings.setValue("board/verticalSplitter",
                        verticalSplitter->saveState());
    }
  }
};

QWidget *Frontend::palettePage() {
  auto p = object(state.value("palette"));
  auto page = new QWidget;
  page->setObjectName("palettePage");
  page->installEventFilter(this);
  auto pageLayout = new QVBoxLayout(page);
  pageLayout->setContentsMargins(0, 0, 0, 0);
  pageLayout->setSpacing(0);

  auto actions = new QToolBar(page);
  actions->setObjectName("paletteActions");
  actions->setMovable(false);
  actions->setFloatable(false);
  actions->setIconSize(QSize(16, 16));
  actions->setFixedHeight(headerBarHeight());
  actions->setContentsMargins(6, 0, 6, 0);
  actions->setProperty("boardAction", true);
  actions->setToolButtonStyle(
      settings.value("board/toolbarLabels", true).toBool()
          ? Qt::ToolButtonTextBesideIcon
          : Qt::ToolButtonIconOnly);
  const QList<QPair<QString, QString>> actionItems{
      {"New draft", "palette_new"},
      {"Save", "palette_save"},
      {"Duplicate", "palette_duplicate"},
      {"Undo", "palette_undo"},
      {"Redo", "palette_redo"}};
  for (int idx = 0; idx < actionItems.size(); ++idx) {
    if (idx == 3)
      actions->addSeparator();
    const auto &pair = actionItems[idx];
    const auto icons = actionIcon(pair.second);
    auto action =
        actions->addAction(themedIcon(icons, actionFallback(pair.second)),
                           pair.first);
    action->setToolTip(actionHint(pair.first, icons.value(0)));
    if (auto *btn = actions->widgetForAction(action))
      btn->setAccessibleName(pair.first);
    connect(action, &QAction::triggered, this, [this, pair] {
      if (pair.second == "palette_save" && tabs)
        commitFocusedEditor(tabs->widget(1));
      send({{"op", pair.second}});
    });
    if (pair.second == "palette_undo")
      action->setEnabled(p.value("can_undo").toBool());
    if (pair.second == "palette_redo")
      action->setEnabled(p.value("can_redo").toBool());
  }
  pageLayout->addWidget(actions);
  auto actionsLine = new Hairline(page);
  actionsLine->setObjectName("paletteActionsHairline");
  pageLayout->addWidget(actionsLine);

  auto w = new QWidget;
  w->setObjectName("paletteContent");
  auto l = new QVBoxLayout(w);
  l->setContentsMargins(12, 10, 12, 14);
  l->setSpacing(10);

  const QFont monoFont = QFontDatabase::systemFont(QFontDatabase::FixedFont);
  const QStringList alignedLabels = {
      "Name:", "Library:", "Mode:", "Base color:", "Count:",
      "Hue range:", "Lightness:", "Chroma:", "Contrast:"};
  int labelW = fontMetrics().horizontalAdvance("Base color:") + 6;
  for (const auto &lbl : alignedLabels)
    labelW = qMax(labelW, fontMetrics().horizontalAdvance(lbl) + 6);
  auto addAlignedRow = [labelW](QFormLayout *form, QWidget *labelParent,
                                const QString &text, QWidget *field,
                                QWidget *buddy = nullptr) {
    auto *lbl = new QLabel(text, labelParent);
    lbl->setMinimumWidth(labelW);
    lbl->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    if (buddy)
      lbl->setBuddy(buddy);
    form->addRow(lbl, field);
    return lbl;
  };
  auto applyMuted = [](QLabel *label, int alpha = 165) {
    QPalette pal = label->palette();
    const QColor muted = mutedColor(pal, alpha);
    pal.setColor(QPalette::WindowText, muted);
    pal.setColor(QPalette::Text, muted);
    label->setPalette(pal);
  };

  // Section 1: Palette & Library
  auto metaForm = new QFormLayout;
  metaForm->setContentsMargins(0, 0, 0, 0);
  metaForm->setHorizontalSpacing(8);
  metaForm->setVerticalSpacing(6);
  metaForm->setLabelAlignment(Qt::AlignRight | Qt::AlignVCenter);
  metaForm->setRowWrapPolicy(QFormLayout::WrapLongRows);
  metaForm->setFieldGrowthPolicy(QFormLayout::AllNonFixedFieldsGrow);

  auto nameRow = new QWidget(w);
  nameRow->setObjectName("paletteNameRow");
  nameRow->installEventFilter(this);
  auto nameRowLayout = new QBoxLayout(QBoxLayout::LeftToRight, nameRow);
  nameRowLayout->setContentsMargins(0, 0, 0, 0);
  nameRowLayout->setSpacing(12);
  const QString initialPaletteName = str(p, "name");
  auto name = line("Palette name", initialPaletteName, nameRow);
  name->setPlaceholderText("Untitled");
  name->setMinimumWidth(110);
  name->setMaximumWidth(260);
  name->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
  connect(name, &QLineEdit::editingFinished, this,
          [this, name, initialPaletteName] {
            if (name->text() == initialPaletteName)
              return;
            send({{"op", "palette_name"}, {"name", name->text()}});
          });
  auto historyLock = new QCheckBox("Lock history additions", nameRow);
  historyLock->setChecked(p.value("lock_history").toBool());
  historyLock->setToolTip(
      "Lock new colors added from history so generation preserves them");
  connect(historyLock, &QCheckBox::toggled, this, [this](bool b) {
    send({{"op", "palette_lock_history"}, {"locked", b}});
  });
  nameRowLayout->addWidget(name, 1);
  nameRowLayout->addWidget(historyLock, 0, Qt::AlignVCenter);
  addAlignedRow(metaForm, w, "Name:", nameRow, name);

  auto libRow = new QWidget(w);
  auto libLayout = new QHBoxLayout(libRow);
  libLayout->setContentsMargins(0, 0, 0, 0);
  libLayout->setSpacing(6);
  const auto savedEntries = array(p, "saved");
  const bool hasSaved = !savedEntries.isEmpty();
  auto saved = combo("Saved palettes", savedEntries, libRow, str(p, "id"));
  saved->setEditable(true);
  saved->setInsertPolicy(QComboBox::NoInsert);
  saved->lineEdit()->setPlaceholderText(QString::fromUtf8("Search palettes\u2026"));
  saved->completer()->setFilterMode(Qt::MatchContains);
  saved->completer()->setCompletionMode(QCompleter::PopupCompletion);
  saved->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
  saved->setMinimumContentsLength(6);
  saved->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
  libLayout->addWidget(saved, 1);

  auto openBtn = button("Open", libRow, [this, saved] {
    send({{"op", "palette_open"}, {"id", saved->currentData().toString()}});
  });
  openBtn->setIcon(themedIcon({"document-open-symbolic", "document-open"},
                              QStyle::SP_DialogOpenButton));
  openBtn->setIconSize(QSize(16, 16));
  openBtn->setToolTip("Open the selected saved palette");
  openBtn->setEnabled(hasSaved);
  libLayout->addWidget(openBtn, 0);

  auto deleteBtn = button("Delete", libRow, [this, saved] {
    if (QMessageBox::question(this, "Delete palette",
                              "Delete the selected saved palette?") ==
        QMessageBox::Yes)
      send({{"op", "palette_delete"}, {"id", saved->currentData().toString()}});
  });
  deleteBtn->setIcon(themedIcon({"edit-delete-symbolic", "edit-delete"},
                                QStyle::SP_TrashIcon));
  deleteBtn->setIconSize(QSize(16, 16));
  deleteBtn->setToolTip("Delete the selected saved palette");
  deleteBtn->setEnabled(hasSaved);
  libLayout->addWidget(deleteBtn, 0);

  addAlignedRow(metaForm, w, "Library:", libRow, saved);
  l->addLayout(metaForm);

  l->addWidget(new Hairline(w));

  // Section 2: Generate palette
  auto genSection = new QVBoxLayout;
  genSection->setObjectName("paletteGenSection");
  genSection->setContentsMargins(0, 0, 0, 0);
  genSection->setSpacing(6);

  auto genTitle = new QLabel("Generate palette", w);
  QFont genTitleFont = genTitle->font();
  genTitleFont.setWeight(QFont::Bold);
  if (genTitleFont.pointSizeF() > 0)
    genTitleFont.setPointSizeF(genTitleFont.pointSizeF() + 1.5);
  else if (genTitleFont.pixelSize() > 0)
    genTitleFont.setPixelSize(genTitleFont.pixelSize() + 2);
  genTitle->setFont(genTitleFont);
  genSection->addWidget(genTitle);

  auto gen = new QWidget(w);
  gen->setObjectName("paletteGenerator");
  gen->installEventFilter(this);
  auto genRoot = new QVBoxLayout(gen);
  genRoot->setContentsMargins(0, 0, 0, 0);
  genRoot->setSpacing(10);

  auto columns = new QBoxLayout(QBoxLayout::LeftToRight);
  columns->setObjectName("paletteGeneratorColumns");
  columns->setContentsMargins(0, 0, 0, 0);
  columns->setSpacing(28);

  auto parameters = new QWidget(gen);
  parameters->setObjectName("paletteParameters");
  parameters->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Preferred);
  auto f = new QFormLayout(parameters);
  f->setContentsMargins(0, 0, 0, 0);
  f->setHorizontalSpacing(8);
  f->setVerticalSpacing(6);
  f->setLabelAlignment(Qt::AlignRight | Qt::AlignVCenter);
  f->setRowWrapPolicy(QFormLayout::WrapLongRows);
  f->setFieldGrowthPolicy(QFormLayout::AllNonFixedFieldsGrow);

  auto selector = new QWidget(gen);
  selector->setObjectName("paletteSelector");
  selector->setMinimumWidth(220);
  selector->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
  auto selectorLayout = new QVBoxLayout(selector);
  selectorLayout->setContentsMargins(0, 0, 0, 0);
  selectorLayout->setSpacing(10);

  columns->addWidget(parameters, 0, Qt::AlignTop);
  columns->addWidget(selector, 1, Qt::AlignTop);
  genRoot->addLayout(columns);

  const int fieldW = 248;

  // Basic parameters group
  auto mode = new QComboBox(parameters);
  mode->setAccessibleName("Generation mode");
  for (auto entry : QList<QPair<QString, QString>>{
           {"Complementary", "complementary"},
           {"Split complementary", "split-complementary"},
           {"Analogous", "analogous"},
           {"Triadic", "triadic"},
           {"Tetradic", "tetradic"},
           {"Monochromatic", "monochromatic"},
           {"Perceptual contrast", "perceptual-contrast"},
           {"Random", "random"}})
    mode->addItem(entry.first, entry.second);
  int modeIndex = mode->findData(generationDraft.value("mode").toString());
  if (modeIndex >= 0)
    mode->setCurrentIndex(modeIndex);
  mode->setFixedWidth(fieldW);
  connect(mode, &QComboBox::currentIndexChanged, this, [this, mode] {
    generationDraft["mode"] = mode->currentData().toString();
  });
  addAlignedRow(f, parameters, "Mode:", mode, mode);

  auto resolveBaseColor = [this](const QString &text) {
    return color(text.isEmpty() ? str(object(state.value("mixer")), "current")
                                : text);
  };
  auto baseRow = new QWidget(parameters);
  baseRow->setFixedWidth(fieldW);
  auto baseRowLayout = new QHBoxLayout(baseRow);
  baseRowLayout->setContentsMargins(0, 0, 0, 0);
  baseRowLayout->setSpacing(6);
  auto base = line("Base color (empty for current)",
                   generationDraft.value("base").toString(), baseRow);
  base->setFont(monoFont);
  base->setPlaceholderText("Current mixer color");
  base->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
  const QColor initialBase = resolveBaseColor(base->text());
  auto baseChip = new ColorChip(initialBase, baseRow);
  baseRowLayout->addWidget(baseChip, 0, Qt::AlignVCenter);
  baseRowLayout->addWidget(base, 1);
  connect(base, &QLineEdit::textChanged, this,
          [this, baseChip, resolveBaseColor](const QString &value) {
            generationDraft["base"] = value;
            baseChip->setColor(resolveBaseColor(value));
          });
  addAlignedRow(f, parameters, "Base color:", baseRow, base);

  auto countSeedRow = new QWidget(parameters);
  countSeedRow->setFixedWidth(fieldW);
  auto countSeedLayout = new QHBoxLayout(countSeedRow);
  countSeedLayout->setContentsMargins(0, 0, 0, 0);
  countSeedLayout->setSpacing(0);
  auto count = spin("Count", generationDraft.value("count").toInt(5), 1, 64,
                    countSeedRow);
  count->setFixedWidth(qMax(58, count->sizeHint().width()));
  connect(count, &QSpinBox::valueChanged, this,
          [this](int value) { generationDraft["count"] = value; });
  auto seedLabel = new QLabel("Seed:", countSeedRow);
  auto seed = line("Random seed", generationDraft.value("seed").toString("0"),
                   countSeedRow);
  seed->setFont(monoFont);
  seed->setPlaceholderText("0");
  seed->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
  seedLabel->setBuddy(seed);
  connect(seed, &QLineEdit::textChanged, this,
          [this](const QString &value) { generationDraft["seed"] = value; });
  auto randomSeedBtn = new QToolButton(countSeedRow);
  randomSeedBtn->setAutoRaise(false);
  const int seedHeight = seed->sizeHint().height();
  randomSeedBtn->setFixedSize(seedHeight, seedHeight);
  randomSeedBtn->setIcon(themedIcon(
      {"roll-symbolic", "roll", "view-refresh-symbolic", "view-refresh"},
      QStyle::SP_BrowserReload));
  randomSeedBtn->setIconSize(QSize(16, 16));
  randomSeedBtn->setToolTip("Randomize seed");
  randomSeedBtn->setAccessibleName("Randomize seed");
  connect(randomSeedBtn, &QToolButton::clicked, this, [seed] {
    seed->setText(QString::number(QRandomGenerator::global()->generate()));
  });
  countSeedLayout->addWidget(count, 0, Qt::AlignVCenter);
  countSeedLayout->addSpacing(10);
  countSeedLayout->addWidget(seedLabel, 0, Qt::AlignVCenter);
  countSeedLayout->addSpacing(6);
  countSeedLayout->addWidget(seed, 1);
  countSeedLayout->addSpacing(4);
  countSeedLayout->addWidget(randomSeedBtn, 0, Qt::AlignVCenter);
  addAlignedRow(f, parameters, "Count:", countSeedRow, count);

  // Subtle 8px extra vertical breathing space before constraint tuning
  f->addItem(new QSpacerItem(0, 2, QSizePolicy::Minimum, QSizePolicy::Fixed));

  // Range & contrast tuning group
  auto hmin = decimal("Minimum hue", generationDraft.value("hmin").toDouble(0),
                      0, 360, parameters),
       hmax = decimal("Maximum hue",
                      generationDraft.value("hmax").toDouble(360), 0, 360,
                      parameters),
       lmin = decimal("Minimum lightness",
                      generationDraft.value("lmin").toDouble(0), 0, 1,
                      parameters),
       lmax = decimal("Maximum lightness",
                      generationDraft.value("lmax").toDouble(1), 0, 1,
                      parameters),
       cmin = decimal("Minimum chroma",
                      generationDraft.value("cmin").toDouble(0), 0, .4,
                      parameters),
       cmax = decimal("Maximum chroma",
                      generationDraft.value("cmax").toDouble(.4), 0, .4,
                      parameters);
  for (auto *hSpin : {hmin, hmax}) {
    hSpin->setDecimals(1);
    hSpin->setSingleStep(1.0);
    hSpin->setSuffix(QString::fromUtf8("°"));
  }
  hmin->setValue(generationDraft.value("hmin").toDouble(0));
  hmax->setValue(generationDraft.value("hmax").toDouble(360));
  for (auto *lcSpin : {lmin, lmax, cmin, cmax}) {
    lcSpin->setDecimals(2);
    lcSpin->setSingleStep(0.01);
  }
  lmin->setValue(generationDraft.value("lmin").toDouble(0));
  lmax->setValue(generationDraft.value("lmax").toDouble(1));
  cmin->setValue(generationDraft.value("cmin").toDouble(0));
  cmax->setValue(generationDraft.value("cmax").toDouble(.4));

  for (auto pair : QList<QPair<QDoubleSpinBox *, QString>>{{hmin, "hmin"},
                                                           {hmax, "hmax"},
                                                           {lmin, "lmin"},
                                                           {lmax, "lmax"},
                                                           {cmin, "cmin"},
                                                           {cmax, "cmax"}}) {
    pair.first->setMinimumWidth(0);
    pair.first->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    connect(
        pair.first, &QDoubleSpinBox::valueChanged, this,
        [this, pair](double value) { generationDraft[pair.second] = value; });
  }
  auto makeRangeRow = [parameters, fieldW, applyMuted](QDoubleSpinBox *lo,
                                                       QDoubleSpinBox *hi) {
    auto row = new QWidget(parameters);
    row->setFixedWidth(fieldW);
    auto rowLayout = new QHBoxLayout(row);
    rowLayout->setContentsMargins(0, 0, 0, 0);
    rowLayout->setSpacing(6);
    lo->setParent(row);
    hi->setParent(row);
    auto dash = new QLabel(QString::fromUtf8("–"), row);
    applyMuted(dash, 165);
    rowLayout->addWidget(lo, 1);
    rowLayout->addWidget(dash, 0, Qt::AlignCenter);
    rowLayout->addWidget(hi, 1);
    return row;
  };
  addAlignedRow(f, parameters, "Hue range:", makeRangeRow(hmin, hmax), hmin);
  addAlignedRow(f, parameters, "Lightness:", makeRangeRow(lmin, lmax), lmin);
  addAlignedRow(f, parameters, "Chroma:", makeRangeRow(cmin, cmax), cmin);

  auto contrastRow = new QWidget(parameters);
  contrastRow->setFixedWidth(fieldW);
  auto contrastLayout = new QHBoxLayout(contrastRow);
  contrastLayout->setContentsMargins(0, 0, 0, 0);
  contrastLayout->setSpacing(4);
  auto contrast = line("Contrast (optional)",
                       generationDraft.value("contrast").toString(),
                       contrastRow);
  contrast->setPlaceholderText("Optional (e.g. 4.5)");
  contrast->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
  connect(
      contrast, &QLineEdit::textChanged, this,
      [this](const QString &value) { generationDraft["contrast"] = value; });
  auto ratioLabel = new QLabel(":1", contrastRow);
  applyMuted(ratioLabel, 165);
  contrastLayout->addWidget(contrast, 1);
  contrastLayout->addWidget(ratioLabel, 0, Qt::AlignVCenter);
  addAlignedRow(f, parameters, "Contrast:", contrastRow, contrast);

  auto wheel = new GeneratorWheel(
      initialBase, [base](QColor c) { base->setText(hex(c)); }, selector);
  wheel->setObjectName("paletteWheel");
  connect(base, &QLineEdit::textChanged, wheel,
          [wheel, resolveBaseColor](const QString &text) {
            wheel->setColor(resolveBaseColor(text));
          });
  wheel->setFixedSize(176, 176);
  selectorLayout->addWidget(wheel, 0, Qt::AlignHCenter);

  auto previewStrip = new PreviewStrip(selector);
  previewStrip->setObjectName("palettePreviewStrip");
  selectorLayout->addWidget(previewStrip);

  auto previewError = new QLabel(selector);
  previewError->setWordWrap(true);
  previewError->hide();
  selectorLayout->addWidget(previewError);

  auto makeRequest = [=, this](const QString &op) -> QJsonObject {
    bool ok = false;
    qulonglong sv = seed->text().toULongLong(&ok);
    if (!ok) return {};
    QJsonValue cv = QJsonValue::Null;
    if (!contrast->text().trimmed().isEmpty()) {
      double x = contrast->text().toDouble(&ok);
      if (!ok) return {};
      cv = x;
    }
    return {{"op", op},
            {"mode", mode->currentData().toString()},
            {"count", count->value()},
            {"seed", QString::number(sv)},
            {"base", base->text().isEmpty()
                         ? str(object(state.value("mixer")), "current")
                         : base->text()},
            {"hue_min", hmin->value()}, {"hue_max", hmax->value()},
            {"lightness_min", lmin->value()}, {"lightness_max", lmax->value()},
            {"chroma_min", cmin->value()}, {"chroma_max", cmax->value()},
            {"contrast", cv}};
  };

  auto updatePreview = [=, this] {
    auto req = makeRequest("palette_preview_generation");
    QJsonObject result =
        req.isEmpty()
            ? QJsonObject{{"error", "Enter a valid seed and contrast."}}
            : request(req).toObject();
    previewError->setText(str(result, "error"));
    previewError->setVisible(!previewError->text().isEmpty());
    QList<QColor> colors;
    QStringList labels;
    for (const auto &value : array(result, "colors")) {
      const QString text = value.toString();
      auto c = color(text);
      if (!c.isValid())
        continue;
      colors.append(c);
      labels.append(text);
    }
    previewStrip->setColors(colors, labels);
    wheel->setColors(colors);
    const auto selectedMode = mode->currentData().toString();
    wheel->setHarmonyHandles(selectedMode != "random" &&
                             selectedMode != "perceptual-contrast");
  };

  auto previewTimer = new QTimer(gen);
  previewTimer->setSingleShot(true);
  previewTimer->setInterval(80);
  connect(previewTimer, &QTimer::timeout, gen, updatePreview);
  auto schedulePreview = [previewTimer] { previewTimer->start(); };
  connect(mode, &QComboBox::currentIndexChanged, gen, schedulePreview);
  connect(count, &QSpinBox::valueChanged, gen, schedulePreview);
  for (auto edit : {base, seed, contrast})
    connect(edit, &QLineEdit::textChanged, gen, schedulePreview);
  for (auto control : {hmin, hmax, lmin, lmax, cmin, cmax})
    connect(control, &QDoubleSpinBox::valueChanged, gen, schedulePreview);

  updatePreview();

  auto generateBtn = new PrimaryActionButton("Generate", selector);
  connect(generateBtn, &QPushButton::clicked, selector, [=, this] {
    auto req = makeRequest("palette_generate");
    if (req.isEmpty()) {
      QMessageBox::warning(
          this, "Invalid generation settings",
          "Enter a valid non-negative integer seed and numeric contrast.");
      return;
    }
    send(req);
  });
  generateBtn->setIcon(themedIcon(
      {"tools-wizard-symbolic", "tools-wizard", "view-refresh-symbolic",
       "view-refresh"},
      QStyle::SP_BrowserReload));
  generateBtn->setIconSize(QSize(16, 16));
  generateBtn->setToolTip("Generate a new palette using these settings");
  generateBtn->setFixedWidth(qMax(168, generateBtn->sizeHint().width()));
  selectorLayout->addWidget(generateBtn, 0, Qt::AlignHCenter);

  // Saved generations recall footer spanning full generator width
  genRoot->addWidget(new Hairline(gen));

  auto savedFooter = new QWidget(gen);
  savedFooter->setObjectName("savedGenerationFooter");
  auto savedFooterLayout = new QBoxLayout(QBoxLayout::LeftToRight, savedFooter);
  savedFooterLayout->setContentsMargins(0, 0, 0, 0);
  savedFooterLayout->setSpacing(8);

  auto savedLabel = new QLabel("Saved generations:", savedFooter);
  savedLabel->setObjectName("savedGenerationLabel");

  auto generationRow = new QWidget(savedFooter);
  generationRow->setObjectName("savedGenerationRow");
  generationRow->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
  auto generationLayout = new QHBoxLayout(generationRow);
  generationLayout->setContentsMargins(0, 0, 0, 0);
  generationLayout->setSpacing(6);
  auto generations =
      combo("Palette generations", array(p, "generations"), generationRow);
  generations->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
  generations->setSizeAdjustPolicy(
      QComboBox::AdjustToMinimumContentsLengthWithIcon);
  generations->setMinimumContentsLength(8);
  int generationIndex =
      generations->findData(generationDraft.value("generation").toString());
  if (generationIndex >= 0)
    generations->setCurrentIndex(generationIndex);
  connect(
      generations, &QComboBox::currentIndexChanged, this, [this, generations] {
        generationDraft["generation"] = generations->currentData().toString();
      });
  savedLabel->setBuddy(generations);
  auto load = toolButton(
      "Load generation", {"document-open-recent-symbolic", "document-open-recent"},
      generationRow,
      [this, generations] {
        send({{"op", "palette_generation"},
              {"id", generations->currentData().toString()}});
      },
      QStyle::SP_DialogOpenButton);
  load->setIconSize(QSize(16, 16));
  bool hasGenerations = generations->count() > 0;
  if (!hasGenerations) {
    generations->addItem("No saved generations");
    generations->setEnabled(false);
    applyMuted(savedLabel, 165);
  }
  load->setEnabled(hasGenerations);
  generationLayout->addWidget(generations, 1);
  generationLayout->addWidget(load, 0);

  savedFooterLayout->addWidget(savedLabel, 0, Qt::AlignVCenter);
  savedFooterLayout->addWidget(generationRow, 1);
  genRoot->addWidget(savedFooter);

  genSection->addWidget(gen);
  l->addLayout(genSection);
  auto status = new QLabel(
      str(p, "error").isEmpty() ? str(p, "status") : str(p, "error"), w);
  status->setWordWrap(true);
  status->setVisible(!status->text().isEmpty());
  l->addWidget(status);
  l->addStretch();

  if (tabs && tabs->width() > 0) {
    const int availW = qMax(1, tabs->width() - 36);
    layoutPaletteGenerator(gen, availW);
    layoutPaletteNameRow(nameRow, qMax(1, availW - labelW - 8));
  }

  pageLayout->addWidget(scroll(w), 1);
  return page;
}

class ThemeSplitter : public QSplitter {
  bool applying = false;

public:
  QList<int> pendingSizes;
  using QSplitter::QSplitter;

protected:
  bool event(QEvent *e) override {
    const bool res = QSplitter::event(e);
    if (!applying && !pendingSizes.isEmpty() &&
        orientation() == Qt::Horizontal && width() >= 680 && count() == 2 &&
        (e->type() == QEvent::LayoutRequest || e->type() == QEvent::Resize)) {
      applying = true;
      setSizes(pendingSizes);
      applying = false;
    }
    return res;
  }
};

void Frontend::layoutThemePage(QWidget *page, int width, bool initialize) {
  auto panes = dynamic_cast<ThemeSplitter *>(
      page->findChild<QSplitter *>("themePanes"));
  auto preview = page->findChild<QWidget *>("themePreviewContainer");
  auto inspector = page->findChild<QScrollArea *>("themeInspector");
  if (!panes || !preview || !inspector)
    return;
  auto previewBody = preview->findChild<QWidget *>("themePreviewBody");
  auto rightLine = preview->findChild<QWidget *>("themePreviewRightLine");
  auto bottomLine = preview->findChild<QWidget *>("themePreviewBottomLine");
  auto canvas = static_cast<Scene *>(preview->findChild<QWidget *>("themeScene"));
  auto renderedBox = page->findChild<QWidget *>("themeRenderedPreviewBox");
  auto inspectorContent = page->findChild<QWidget *>("themeInspectorContent");
  auto inspectorLayout =
      inspectorContent
          ? qobject_cast<QVBoxLayout *>(inspectorContent->layout())
          : nullptr;
  auto exportHairline = page->findChild<QWidget *>("themeExportHairline");
  auto toggle = page->findChild<QAction *>("themePreviewAction");
  auto contextSpacer = page->findChild<QAction *>("themeContextSpacerAction");
  auto contextAction = page->findChild<QAction *>("themeContextAction");
  auto contextLabel = page->findChild<QLabel *>("themeContextLabel");
  const bool compact = width < 680;
  const auto orientation = compact ? Qt::Vertical : Qt::Horizontal;
  if (toggle)
    toggle->setVisible(compact && canvas);
  const bool showContext =
      !compact && contextLabel && !contextLabel->text().isEmpty();
  if (contextSpacer)
    contextSpacer->setVisible(showContext);
  if (contextAction)
    contextAction->setVisible(showContext);
  if (rightLine)
    rightLine->setVisible(!compact);
  if (bottomLine)
    bottomLine->setVisible(compact);
  if (!initialize && panes->orientation() == orientation) {
    preview->setVisible(canvas && (!compact || compactThemePreview));
    return;
  }

  auto previewLayout =
      previewBody ? static_cast<QBoxLayout *>(previewBody->layout()) : nullptr;

  if (compact) {
    panes->pendingSizes.clear();
    if (inspectorContent && inspectorLayout) {
      if (preview->parentWidget() != inspectorContent) {
        preview->setParent(inspectorContent);
        inspectorLayout->insertWidget(0, preview);
      } else if (inspectorLayout->indexOf(preview) < 0) {
        inspectorLayout->insertWidget(0, preview);
      }
      inspectorLayout->setContentsMargins(12, 6, 12, 12);
      inspectorLayout->setSpacing(6);
    }
    panes->setOrientation(Qt::Vertical);
    panes->setHandleWidth(0);
    if (panes->count() > 1 && panes->handle(1))
      panes->handle(1)->setEnabled(false);
    inspector->setMinimumWidth(0);

    preview->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    preview->setMinimumHeight(0);
    preview->setMaximumHeight(QWIDGETSIZE_MAX);
    if (previewBody) {
      previewBody->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
      previewBody->setMinimumHeight(0);
      previewBody->setMaximumHeight(QWIDGETSIZE_MAX);
    }
    if (previewLayout) {
      previewLayout->setDirection(QBoxLayout::TopToBottom);
      previewLayout->setContentsMargins(0, 0, 0, 5);
      previewLayout->setSpacing(4);
    }
    if (canvas) {
      canvas->setMaximumWidth(QWIDGETSIZE_MAX);
      canvas->setTopAligned(false);
      canvas->setCompactMode(true);
      canvas->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
      canvas->setFixedHeight(78);
    }
  } else {
    if (inspectorLayout) {
      inspectorLayout->removeWidget(preview);
      inspectorLayout->setContentsMargins(12, 10, 12, 12);
      inspectorLayout->setSpacing(10);
    }
    if (preview->parentWidget() != panes)
      panes->insertWidget(0, preview);
    panes->setOrientation(Qt::Horizontal);
    panes->setHandleWidth(style()->pixelMetric(QStyle::PM_SplitterWidth));
    if (panes->count() > 1 && panes->handle(1))
      panes->handle(1)->setEnabled(true);
    panes->setChildrenCollapsible(false);
    inspector->setMinimumWidth(200);

    preview->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
    preview->setMinimumHeight(0);
    preview->setMaximumHeight(QWIDGETSIZE_MAX);
    if (previewBody) {
      previewBody->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
      previewBody->setMinimumHeight(0);
      previewBody->setMaximumHeight(QWIDGETSIZE_MAX);
    }
    if (previewLayout) {
      previewLayout->setDirection(QBoxLayout::TopToBottom);
      previewLayout->setContentsMargins(12, 10, 10, 10);
      previewLayout->setSpacing(10);
    }
    if (canvas) {
      canvas->setMaximumWidth(QWIDGETSIZE_MAX);
      canvas->setTopAligned(true);
      canvas->setCompactMode(false);
      canvas->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
      canvas->setMinimumHeight(68);
      canvas->setMaximumHeight(QWIDGETSIZE_MAX);
    }
  }

  preview->setVisible(canvas && (!compact || compactThemePreview));

  const bool previewInLeft = !compact && canvas && previewBody && previewLayout;
  if (renderedBox) {
    if (previewInLeft) {
      if (inspectorLayout)
        inspectorLayout->removeWidget(renderedBox);
      if (renderedBox->parentWidget() != previewBody)
        renderedBox->setParent(previewBody);
      if (previewLayout->indexOf(renderedBox) < 0)
        previewLayout->addWidget(renderedBox, 1);
      else
        previewLayout->setStretchFactor(renderedBox, 1);
      if (canvas)
        previewLayout->setStretchFactor(canvas, 2);
      for (auto *te : renderedBox->findChildren<QPlainTextEdit *>()) {
        te->setMinimumHeight(40);
        te->setMaximumHeight(QWIDGETSIZE_MAX);
      }
      renderedBox->show();
    } else if (inspectorContent && inspectorLayout) {
      if (previewLayout)
        previewLayout->removeWidget(renderedBox);
      if (renderedBox->parentWidget() != inspectorContent)
        renderedBox->setParent(inspectorContent);
      if (inspectorLayout->indexOf(renderedBox) < 0) {
        const int idx = exportHairline
                            ? inspectorLayout->indexOf(exportHairline)
                            : inspectorLayout->count();
        inspectorLayout->insertWidget(idx >= 0 ? idx : inspectorLayout->count(),
                                      renderedBox);
      }
      if (canvas && previewLayout)
        previewLayout->setStretchFactor(canvas, 0);
      for (auto *te : renderedBox->findChildren<QPlainTextEdit *>()) {
        const int h = te->property("compactHeight").toInt();
        te->setFixedHeight(h > 0 ? h : 96);
      }
      renderedBox->show();
    }
  } else if (canvas && previewLayout) {
    previewLayout->setStretchFactor(canvas, compact ? 0 : 1);
  }

  if (!compact) {
    const auto saved = settings.value("board/themeSplitter").toList();
    const QList<int> target =
        saved.size() == 2 ? QList<int>{saved[0].toInt(), saved[1].toInt()}
                          : QList<int>{416, 364};
    if (canvas)
      canvas->syncWideMaxHeight(qMax(canvas->width(), qMax(140, target[0] - 23)));
    panes->pendingSizes = target;
    panes->setSizes(target);
  }
}

QWidget *Frontend::createThemePage() {
  themeContainer = new QWidget;
  themeContainer->setObjectName("themePage");
  themeContainer->installEventFilter(this);
  auto containerLayout = new QVBoxLayout(themeContainer);
  containerLayout->setSpacing(0);
  containerLayout->setContentsMargins(0, 0, 0, 0);

  // Persistent Theme Project Dialog owned by themeContainer so editing
  // Theme name and clicking Close / New theme / Open never tears down the
  // dialog mid-click.
  auto projectDialog = new QDialog(themeContainer);
  themeProjectDialog = projectDialog;
  projectDialog->setObjectName("themeProjectDialog");
  projectDialog->setWindowTitle("Theme project");
  auto projectLayout = new QVBoxLayout(projectDialog);
  projectLayout->setContentsMargins(14, 12, 14, 12);
  projectLayout->setSpacing(10);

  auto selection = new QFormLayout;
  selection->setContentsMargins(0, 0, 0, 0);
  selection->setHorizontalSpacing(8);
  selection->setVerticalSpacing(8);
  selection->setLabelAlignment(Qt::AlignRight | Qt::AlignVCenter);
  selection->setRowWrapPolicy(QFormLayout::WrapLongRows);
  selection->setFieldGrowthPolicy(QFormLayout::AllNonFixedFieldsGrow);

  auto makeDialogButton = [this](const QString &label, const QStringList &icons,
                                 QStyle::StandardPixmap fallback, QWidget *parent,
                                 const std::function<void()> &action) {
    auto *b = new QPushButton(themedIcon(icons, fallback), label, parent);
    b->setAccessibleName(label);
    b->setIconSize(QSize(16, 16));
    b->setAutoDefault(false);
    b->setDefault(false);
    connect(b, &QPushButton::clicked, this, action);
    return b;
  };

  auto discard = [this] {
    const auto proj = object(object(state.value("theme")).value("project"));
    return !proj.value("dirty").toBool() ||
           QMessageBox::question(this, "Unsaved theme",
                                 "Discard unsaved theme changes?") ==
               QMessageBox::Yes;
  };

  auto plugins = new QComboBox(projectDialog);
  plugins->setAccessibleName("Theme plugins");
  plugins->setMinimumContentsLength(12);
  plugins->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
  themePluginsCombo = plugins;
  connect(plugins, &QComboBox::currentIndexChanged, this, [this, plugins] {
    themeDraft["plugin"] = plugins->currentData().toString();
  });
  auto pluginRow = new QWidget(projectDialog);
  auto pluginLayout = new QHBoxLayout(pluginRow);
  pluginLayout->setContentsMargins(0, 0, 0, 0);
  pluginLayout->setSpacing(6);
  pluginLayout->addWidget(plugins, 1);
  auto newThemeBtn = makeDialogButton(
      "New theme", {"document-new-symbolic", "document-new"}, QStyle::SP_FileIcon,
      pluginRow, [this, plugins, discard] {
        if (discard())
          send({{"op", "theme_choose"},
                {"id", plugins->currentData().toString()},
                {"discard", true}});
      });
  themeNewBtn = newThemeBtn;
  pluginLayout->addWidget(newThemeBtn);
  selection->addRow("Plugin:", pluginRow);

  auto saved = new QComboBox(projectDialog);
  saved->setAccessibleName("Saved themes");
  saved->setMinimumContentsLength(12);
  saved->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
  themeSavedCombo = saved;
  connect(saved, &QComboBox::currentIndexChanged, this, [this, saved] {
    if (saved->isEnabled())
      themeDraft["saved"] = saved->currentData().toString();
  });
  auto savedRow = new QWidget(projectDialog);
  auto savedLayout = new QHBoxLayout(savedRow);
  savedLayout->setContentsMargins(0, 0, 0, 0);
  savedLayout->setSpacing(6);
  savedLayout->addWidget(saved, 1);
  auto openBtn = makeDialogButton(
      "Open", {"document-open-symbolic", "document-open"},
      QStyle::SP_DialogOpenButton, savedRow, [this, saved, discard] {
        if (discard())
          send({{"op", "theme_open"},
                {"id", saved->currentData().toString()},
                {"discard", true}});
      });
  openBtn->setAccessibleName("Open theme");
  themeOpenBtn = openBtn;
  savedLayout->addWidget(openBtn);
  auto reloadBtn = new QToolButton(savedRow);
  reloadBtn->setIcon(themedIcon({"view-refresh-symbolic", "view-refresh"},
                                QStyle::SP_BrowserReload));
  reloadBtn->setIconSize(QSize(16, 16));
  reloadBtn->setAutoRaise(false);
  const int reloadSide = qMax(28, saved->sizeHint().height());
  reloadBtn->setFixedSize(reloadSide, reloadSide);
  reloadBtn->setAccessibleName("Reload");
  reloadBtn->setToolTip("Reload themes and plugins");
  connect(reloadBtn, &QToolButton::clicked, this,
          [this] { send({{"op", "theme_reload"}}); });
  savedLayout->addWidget(reloadBtn);
  const int rightColW = qMax(newThemeBtn->sizeHint().width(),
                             openBtn->sizeHint().width() + 6 + reloadSide);
  newThemeBtn->setMinimumWidth(rightColW);
  openBtn->setMinimumWidth(
      qMax(openBtn->sizeHint().width(), rightColW - 6 - reloadSide));
  selection->addRow("Saved:", savedRow);

  auto name = line("Theme name", QString(), projectDialog);
  name->setPlaceholderText("Theme name");
  name->setMaximumWidth(
      name->fontMetrics().horizontalAdvance(QString(32, QLatin1Char('M'))) + 20);
  themeNameEdit = name;
  connect(name, &QLineEdit::editingFinished, this, [this, name] {
    const auto t = object(state.value("theme"));
    const bool hasProject =
        t.value("project").isObject() && !t.value("project").toObject().isEmpty();
    if (!hasProject)
      return;
    const QString currentName = str(t.value("project").toObject(), "name");
    if (name->text() == currentName)
      return;
    send({{"op", "theme_name"}, {"name", name->text()}});
  });
  selection->addRow("Name:", name);

  projectLayout->addLayout(selection);
  projectLayout->addWidget(new Hairline(projectDialog));
  auto closeProject =
      new QDialogButtonBox(QDialogButtonBox::Close, projectDialog);
  if (auto *closeBtn = closeProject->button(QDialogButtonBox::Close)) {
    closeBtn->setAutoDefault(false);
    closeBtn->setDefault(false);
  }
  connect(closeProject, &QDialogButtonBox::rejected, projectDialog,
          [this, projectDialog, name] {
            if (name && name->isEnabled() && name->isModified()) {
              QFocusEvent focusOut(QEvent::FocusOut, Qt::OtherFocusReason);
              QApplication::sendEvent(name, &focusOut);
            }
            projectDialog->reject();
          });
  connect(projectDialog, &QDialog::finished, this,
          [this] { themeProjectVisible = false; });
  projectLayout->addWidget(closeProject);

  syncThemeProjectDialog();
  refreshThemeContent(nullptr);
  return themeContainer;
}

void Frontend::syncThemeProjectDialog() {
  if (!themeProjectDialog)
    return;
  const auto t = object(state.value("theme"));
  const bool hasProject =
      t.value("project").isObject() && !t.value("project").toObject().isEmpty();
  const auto project = hasProject ? t.value("project").toObject() : QJsonObject{};

  if (themePluginsCombo) {
    QSignalBlocker block(themePluginsCombo);
    const QString desired =
        themeDraft.value("plugin").toString(str(project, "plugin_id"));
    themePluginsCombo->clear();
    const auto pluginEntries = array(t, "plugins");
    for (auto v : pluginEntries) {
      auto o = v.toObject();
      themePluginsCombo->addItem(
          str(o, "name").isEmpty() ? str(o, "label") : str(o, "name"),
          str(o, "id"));
    }
    const int idx = themePluginsCombo->findData(desired);
    if (idx >= 0)
      themePluginsCombo->setCurrentIndex(idx);
    if (themeNewBtn)
      themeNewBtn->setEnabled(!pluginEntries.isEmpty());
  }
  if (themeSavedCombo) {
    QSignalBlocker block(themeSavedCombo);
    const auto savedEntries = array(t, "saved");
    const bool hasSaved = !savedEntries.isEmpty();
    const QString desired = themeDraft.value("saved").toString();
    themeSavedCombo->clear();
    if (!hasSaved) {
      themeSavedCombo->addItem("No saved themes");
      themeSavedCombo->setEnabled(false);
    } else {
      for (auto v : savedEntries) {
        auto o = v.toObject();
        themeSavedCombo->addItem(
            str(o, "name").isEmpty() ? str(o, "label") : str(o, "name"),
            str(o, "id"));
      }
      themeSavedCombo->setEnabled(true);
      const int idx = themeSavedCombo->findData(desired);
      if (idx >= 0)
        themeSavedCombo->setCurrentIndex(idx);
    }
    if (themeOpenBtn)
      themeOpenBtn->setEnabled(hasSaved);
  }
  if (themeNameEdit) {
    QSignalBlocker block(themeNameEdit);
    const QString desiredName = hasProject ? str(project, "name") : QString();
    themeNameEdit->setEnabled(hasProject);
    if (themeNameEdit->text() != desiredName) {
      themeNameEdit->setText(desiredName);
      themeNameEdit->setModified(false);
    }
  }
  if (themeProjectVisible && !themeProjectDialog->isVisible())
    themeProjectDialog->show();
}

void Frontend::refreshThemeContent(Scene *reuse) {
  if (!themeContainer)
    return;
  auto *layout = qobject_cast<QVBoxLayout *>(themeContainer->layout());
  if (!layout)
    return;
  auto *newBody = buildThemeBody(reuse);
  auto *oldBody = themeBody;
  if (oldBody) {
    preserveView(oldBody, newBody);
    suppressChildSignals(oldBody);
    layout->removeWidget(oldBody);
    oldBody->hide();
    oldBody->setParent(nullptr);
    oldBody->deleteLater();
  }
  themeBody = newBody;
  layout->addWidget(newBody, 1);
  const int targetW = themeContainer->width() > 0
                          ? themeContainer->width()
                          : (tabs ? tabs->width() : width());
  layoutThemePage(themeContainer, targetW, true);
  alignHeaderToolbar(themeContainer, "themeActions");
  if (themeContainer->isVisible()) {
    newBody->show();
    layout->activate();
  }
}

QWidget *Frontend::buildThemeBody(Scene *reuse) {
  auto t = object(state.value("theme"));
  const bool hasProject =
      t.value("project").isObject() && !t.value("project").toObject().isEmpty();
  auto project = hasProject ? t.value("project").toObject() : QJsonObject{};
  auto outer = new QWidget(themeContainer);
  outer->setObjectName("themeBody");
  auto outerLayout = new QVBoxLayout(outer);
  outerLayout->setSpacing(0);
  outerLayout->setContentsMargins(0, 0, 0, 0);

  const QFont monoFont = QFontDatabase::systemFont(QFontDatabase::FixedFont);
  const QFontMetrics monoFm(monoFont);
  auto applyMuted = [](QLabel *label, int alpha = 165) {
    QPalette pal = label->palette();
    const QColor muted = mutedColor(pal, alpha);
    pal.setColor(QPalette::WindowText, muted);
    pal.setColor(QPalette::Text, muted);
    label->setPalette(pal);
  };

  // Pinned Theme Action Toolbar (flush with History header hairline)
  auto actions = new QToolBar(outer);
  actions->setObjectName("themeActions");
  actions->setMovable(false);
  actions->setFloatable(false);
  actions->setIconSize(QSize(16, 16));
  actions->setFixedHeight(headerBarHeight());
  actions->setContentsMargins(6, 0, 6, 0);
  actions->setProperty("boardAction", true);
  actions->setToolButtonStyle(
      settings.value("board/toolbarLabels", true).toBool()
          ? Qt::ToolButtonTextBesideIcon
          : Qt::ToolButtonIconOnly);

  auto addThemeAction = [actions, this](const QString &label,
                                        const QStringList &icons,
                                        QStyle::StandardPixmap fallback,
                                        const QString &tooltip,
                                        const std::function<void()> &onTrigger) {
    auto *act = actions->addAction(themedIcon(icons, fallback), label);
    act->setToolTip(tooltip);
    if (auto *btn = qobject_cast<QToolButton *>(actions->widgetForAction(act))) {
      btn->setAccessibleName(label);
      btn->setFocusPolicy(Qt::TabFocus);
    }
    if (onTrigger)
      connect(act, &QAction::triggered, this, onTrigger);
    return act;
  };

  addThemeAction("Project",
                 {"project-open-symbolic", "document-open-folder-symbolic",
                  "folder-open"},
                 QStyle::SP_DirOpenIcon, "Theme project settings",
                 [this] {
                   themeProjectVisible = true;
                   syncThemeProjectDialog();
                   if (themeProjectDialog) {
                     themeProjectDialog->show();
                     themeProjectDialog->raise();
                     themeProjectDialog->activateWindow();
                   }
                 });

  auto previewToggleAction = actions->addAction(
      themedIcon({"view-preview-symbolic", "view-preview"},
                 QStyle::SP_FileDialogContentsView),
      "Show preview");
  previewToggleAction->setObjectName("themePreviewAction");
  previewToggleAction->setToolTip("Show or hide theme preview");
  previewToggleAction->setCheckable(true);
  previewToggleAction->setChecked(compactThemePreview);
  if (auto *previewToggle = qobject_cast<QToolButton *>(
          actions->widgetForAction(previewToggleAction))) {
    previewToggle->setObjectName("themePreviewToggle");
    previewToggle->setAccessibleName("Show preview");
    previewToggle->setProperty("boardAction", true);
    previewToggle->setFocusPolicy(Qt::TabFocus);
  }

  auto saveAct = addThemeAction(
      "Save", actionIcon("theme_save"), QStyle::SP_DialogSaveButton, "Save",
      [this] {
        commitFocusedEditor(themeBody);
        send({{"op", "theme_save"}});
      });
  saveAct->setEnabled(hasProject);
  actions->addSeparator();
  auto undoAct = addThemeAction(
      "Undo", actionIcon("theme_undo"), QStyle::SP_ArrowBack,
      actionHint("Undo", "edit-undo-symbolic"),
      [this] { send({{"op", "theme_undo"}}); });
  undoAct->setEnabled(t.value("can_undo").toBool());
  auto redoAct = addThemeAction(
      "Redo", actionIcon("theme_redo"), QStyle::SP_ArrowForward,
      actionHint("Redo", "edit-redo-symbolic"),
      [this] { send({{"op", "theme_redo"}}); });
  redoAct->setEnabled(t.value("can_redo").toBool());
  actions->addSeparator();
  auto bindAct = addThemeAction("Use active palette",
                                actionIcon("theme_palette_bind"),
                                QStyle::SP_DialogApplyButton,
                                "Bind matching roles to the active palette",
                                [this] { send({{"op", "theme_palette_bind"}}); });
  bindAct->setEnabled(hasProject);

  // Subtle right-aligned project context label (wide mode only)
  auto contextSpacer = new QWidget(actions);
  contextSpacer->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
  auto contextSpacerAction = actions->addWidget(contextSpacer);
  contextSpacerAction->setObjectName("themeContextSpacerAction");

  QString pluginName;
  const QString projectPluginId = str(project, "plugin_id");
  for (auto v : array(t, "plugins")) {
    auto po = v.toObject();
    if (str(po, "id") == projectPluginId) {
      pluginName = str(po, "name");
      break;
    }
  }
  if (pluginName.isEmpty())
    pluginName = projectPluginId;
  QString contextText = str(project, "name");
  if (!contextText.isEmpty() && project.value("dirty").toBool())
    contextText += "*";
  if (!pluginName.isEmpty())
    contextText = contextText.isEmpty()
                      ? pluginName
                      : contextText + QStringLiteral(" · ") + pluginName;

  auto contextLabel = new QLabel(contextText, actions);
  contextLabel->setObjectName("themeContextLabel");
  contextLabel->setContentsMargins(6, 0, 6, 0);
  contextLabel->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
  applyMuted(contextLabel, 165);
  auto contextAction = actions->addWidget(contextLabel);
  contextAction->setObjectName("themeContextAction");

  outerLayout->addWidget(actions);
  auto actionsLine = new Hairline(outer);
  actionsLine->setObjectName("themeActionsHairline");
  outerLayout->addWidget(actionsLine);

  // Split Preview & Inspector
  auto panes = new ThemeSplitter(Qt::Vertical, outer);
  panes->setObjectName("themePanes");
  panes->setChildrenCollapsible(false);

  auto previewContainer = new QWidget(panes);
  previewContainer->setObjectName("themePreviewContainer");
  previewContainer->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
  previewContainer->setMinimumHeight(0);
  auto previewGrid = new QGridLayout(previewContainer);
  previewGrid->setContentsMargins(0, 0, 0, 0);
  previewGrid->setSpacing(0);

  auto previewBody = new QWidget(previewContainer);
  previewBody->setObjectName("themePreviewBody");
  previewBody->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
  previewBody->setMinimumHeight(0);
  auto previewLayout = new QBoxLayout(QBoxLayout::TopToBottom, previewBody);
  previewLayout->setContentsMargins(12, 10, 10, 10);
  previewLayout->setSpacing(10);

  auto scene = combo("Preview scene", array(t, "scenes"), previewBody,
                     str(project, "scene"));
  scene->setSizeAdjustPolicy(QComboBox::AdjustToContents);
  scene->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Fixed);
  connect(scene, &QComboBox::activated, this, [this, scene](int) {
    send({{"op", "theme_scene"}, {"id", scene->currentData().toString()}});
  });
  auto sceneSelector = new QWidget(previewBody);
  auto sceneControls = new QHBoxLayout(sceneSelector);
  sceneControls->setContentsMargins(0, 0, 0, 0);
  sceneControls->setSpacing(8);
  auto sceneLabel = new QLabel("Preview scene:", sceneSelector);
  sceneLabel->setBuddy(scene);
  sceneControls->addWidget(sceneLabel);
  sceneControls->addWidget(scene);
  sceneControls->addStretch();
  previewLayout->addWidget(sceneSelector, 0, Qt::AlignTop);

  const auto roleList = array(t, "roles");
  const QString selectedId = str(t, "selected");
  QString selectedTitle;
  for (const auto &rv : roleList) {
    const auto ro = rv.toObject();
    if (str(ro, "id") == selectedId) {
      selectedTitle = str(ro, "name");
      break;
    }
  }

  if (t.value("scene").isObject()) {
    Scene *preview;
    if (reuse) {
      reuse->setParent(previewBody);
      preview = reuse;
    } else {
      preview = new Scene(
          t.value("scene").toObject(), [this](QJsonObject o) { send(o); },
          previewBody);
    }
    preview->setSelectedRole(selectedId, selectedTitle);
    preview->setObjectName("themeScene");
    previewLayout->addWidget(preview, 1);
  }

  auto previewRightLine = new Hairline(Qt::Vertical, previewContainer);
  previewRightLine->setObjectName("themePreviewRightLine");
  auto previewBottomLine = new Hairline(Qt::Horizontal, previewContainer);
  previewBottomLine->setObjectName("themePreviewBottomLine");
  previewGrid->addWidget(previewBody, 0, 0);
  previewGrid->addWidget(previewRightLine, 0, 1);
  previewGrid->addWidget(previewBottomLine, 1, 0, 1, 2);
  previewGrid->setRowStretch(0, 1);
  previewGrid->setColumnStretch(0, 1);

  // Flat KDE HIG Inspector
  auto w = new QWidget;
  w->setObjectName("themeInspectorContent");
  auto l = new QVBoxLayout(w);
  l->setContentsMargins(12, 10, 12, 12);
  l->setSpacing(10);

  // Section 1: Theme roles
  auto rolesSection = new QVBoxLayout;
  rolesSection->setContentsMargins(0, 0, 0, 0);
  rolesSection->setSpacing(6);

  auto rolesHeading = new QLabel("Theme roles", w);
  QFont rolesHeadingFont = rolesHeading->font();
  rolesHeadingFont.setWeight(QFont::Bold);
  if (rolesHeadingFont.pointSizeF() > 0)
    rolesHeadingFont.setPointSizeF(rolesHeadingFont.pointSizeF() + 1.0);
  else if (rolesHeadingFont.pixelSize() > 0)
    rolesHeadingFont.setPixelSize(rolesHeadingFont.pixelSize() + 1);
  rolesHeading->setFont(rolesHeadingFont);
  rolesSection->addWidget(rolesHeading);

  if (roleList.isEmpty()) {
    auto emptyRoles = new QLabel("Open Project to create or load a theme.", w);
    emptyRoles->setWordWrap(true);
    applyMuted(emptyRoles, 155);
    rolesSection->addWidget(emptyRoles);
  } else {
    auto rolesListLayout = new QVBoxLayout;
    rolesListLayout->setContentsMargins(0, 0, 0, 0);
    rolesListLayout->setSpacing(4);
    for (int roleIdx = 0; roleIdx < roleList.size(); ++roleIdx) {
      if (roleIdx > 0)
        rolesListLayout->addWidget(new Hairline(w));
      auto o = roleList[roleIdx].toObject();
      auto id = str(o, "id");
      const bool selected = selectedId == id;
      auto row = new ThemeRoleCard(
          selected, [this, id] { send({{"op", "theme_select"}, {"role", id}}); },
          w);
      if (selected)
        row->setObjectName("selectedThemeRole");
      auto r = new QVBoxLayout(row);
      r->setContentsMargins(6, 6, 6, 6);
      r->setSpacing(5);

      auto topRow = new QHBoxLayout;
      topRow->setContentsMargins(0, 0, 0, 0);
      topRow->setSpacing(6);
      auto sw = new Swatch(color(str(o, "color")), str(o, "name"), row);
      sw->setFlat(true);
      sw->setCursor(Qt::PointingHandCursor);
      if (selected) {
        QFont swFont = sw->font();
        swFont.setWeight(QFont::DemiBold);
        sw->setFont(swFont);
      }
      const QString helpText = str(o, "help");
      sw->setToolTip(helpText.isEmpty()
                         ? "Click to select role; drag to copy color"
                         : helpText);
      topRow->addWidget(sw, 0, Qt::AlignLeft | Qt::AlignVCenter);
      topRow->addStretch(1);
      connect(sw, &QPushButton::clicked, this,
              [this, id] { send({{"op", "theme_select"}, {"role", id}}); });

      const QString initialRoleValue = str(o, "value");
      auto edit = new ColorLineEdit(initialRoleValue, row);
      edit->setFont(monoFont);
      edit->setMinimumWidth(monoFm.horizontalAdvance("#142638") + 16);
      edit->setMaximumWidth(monoFm.horizontalAdvance("#142638FF") + 18);
      topRow->addWidget(edit, 0, Qt::AlignRight | Qt::AlignVCenter);
      connect(edit, &QLineEdit::editingFinished, this,
              [this, id, edit, initialRoleValue] {
                if (edit->text() == initialRoleValue)
                  return;
                send({{"op", "theme_assign"},
                      {"role", id},
                      {"color", edit->text()}});
              });
      r->addLayout(topRow);

      auto bottomRow = new QHBoxLayout;
      bottomRow->setContentsMargins(0, 0, 0, 0);
      bottomRow->setSpacing(6);
      auto linkLabel = new QLabel("Palette link:", row);
      applyMuted(linkLabel, 175);
      auto link = new QComboBox(row);
      link->setAccessibleName("Palette swatch link");
      link->setEditable(true);
      link->setInsertPolicy(QComboBox::NoInsert);
      if (auto *le = link->lineEdit()) {
        le->setPlaceholderText("Unlinked");
        QPalette lePal = le->palette();
        lePal.setColor(QPalette::PlaceholderText, mutedColor(lePal, 185));
        le->setPalette(lePal);
      }
      link->addItem("Unlinked", QString());
      for (auto v : array(t, "palette_swatches")) {
        const QString swatchName = str(v.toObject(), "name");
        link->addItem(swatchName, swatchName);
      }
      const QString initialLink = str(o, "link");
      if (initialLink.isEmpty()) {
        link->setCurrentIndex(0);
        link->setEditText(QString());
      } else {
        const int idx = link->findData(initialLink);
        if (idx >= 0)
          link->setCurrentIndex(idx);
        else
          link->setEditText(initialLink);
      }
      link->setMinimumContentsLength(7);
      link->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
      link->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
      linkLabel->setBuddy(link);
      bottomRow->addWidget(linkLabel, 0, Qt::AlignLeft | Qt::AlignVCenter);
      bottomRow->addWidget(link, 1);

      auto defaultBtn = new QToolButton(row);
      defaultBtn->setIcon(themedIcon({"edit-undo-symbolic", "edit-undo"},
                                     QStyle::SP_ArrowBack));
      defaultBtn->setIconSize(QSize(16, 16));
      defaultBtn->setAutoRaise(true);
      defaultBtn->setFixedSize(26, 26);
      defaultBtn->setAccessibleName("Default");
      defaultBtn->setToolTip("Reset role to default");
      defaultBtn->setEnabled(str(o, "assignment") != "default");
      connect(defaultBtn, &QToolButton::clicked, this, [this, id] {
        send({{"op", "theme_default"}, {"role", id}});
      });
      bottomRow->addWidget(defaultBtn, 0, Qt::AlignRight | Qt::AlignVCenter);
      r->addLayout(bottomRow);

      auto committedLink = std::make_shared<QString>(initialLink);
      auto commitLink = [this, id, committedLink](const QString &rawName) {
        const QString nextName = rawName.trimmed();
        if (nextName == *committedLink)
          return;
        *committedLink = nextName;
        send({{"op", "theme_link"}, {"role", id}, {"name", nextName}});
      };
      connect(link, &QComboBox::activated, this,
              [link, commitLink](int idx) {
                const QString val = (idx >= 0 && link->itemData(idx).isValid())
                                        ? link->itemData(idx).toString()
                                        : link->currentText();
                if (val.isEmpty())
                  link->setEditText(QString());
                commitLink(val);
              });
      if (auto *le = link->lineEdit()) {
        connect(le, &QLineEdit::editingFinished, this, [le, commitLink] {
          commitLink(le->text());
        });
      }
      rolesListLayout->addWidget(row);
    }
    rolesSection->addLayout(rolesListLayout);
  }
  l->addLayout(rolesSection);

  // Section 2: Rendered Output Preview (left pane at wide widths, inspector in compact mode)
  const auto previews = array(t, "preview");
  if (!previews.isEmpty()) {
    auto renderedPreviewBox = new QWidget(w);
    renderedPreviewBox->setObjectName("themeRenderedPreviewBox");
    auto renderedLayout = new QVBoxLayout(renderedPreviewBox);
    renderedLayout->setContentsMargins(0, 0, 0, 0);
    renderedLayout->setSpacing(6);
    renderedLayout->addWidget(new Hairline(renderedPreviewBox));
    for (int i = 0; i < previews.size(); ++i) {
      if (i > 0)
        renderedLayout->addWidget(new Hairline(renderedPreviewBox));
      auto o = previews[i].toObject();
      auto previewSection = new QVBoxLayout;
      previewSection->setContentsMargins(0, 0, 0, 0);
      previewSection->setSpacing(5);

      auto headerRow = new QHBoxLayout;
      headerRow->setContentsMargins(0, 0, 0, 0);
      headerRow->setSpacing(8);
      auto pathLabel = new QLabel(str(o, "path"), renderedPreviewBox);
      QFont pathFont = pathLabel->font();
      pathFont.setWeight(QFont::DemiBold);
      pathLabel->setFont(pathFont);
      headerRow->addWidget(pathLabel);
      headerRow->addStretch(1);
      auto badge = new QLabel("Preview", renderedPreviewBox);
      QFont badgeFont = badge->font();
      if (badgeFont.pointSizeF() > 0)
        badgeFont.setPointSizeF(qMax(8.0, badgeFont.pointSizeF() - 1.0));
      else if (badgeFont.pixelSize() > 0)
        badgeFont.setPixelSize(qMax(9, badgeFont.pixelSize() - 1));
      badge->setFont(badgeFont);
      applyMuted(badge, 155);
      headerRow->addWidget(badge);
      previewSection->addLayout(headerRow);

      const QString content = str(o, "content");
      auto text = new QPlainTextEdit(content, renderedPreviewBox);
      text->setReadOnly(true);
      text->setTabChangesFocus(true);
      text->setAccessibleName("Rendered " + str(o, "path"));
      text->setFont(monoFont);
      text->document()->setDocumentMargin(6);
      text->setFrameShape(QFrame::StyledPanel);
      const int lineCount =
          qMax(1, content.count('\n') + (content.endsWith('\n') ? 0 : 1));
      const int compactH = qBound(86, lineCount * monoFm.lineSpacing() + 30, 136);
      text->setProperty("compactHeight", compactH);
      text->setFixedHeight(compactH);
      previewSection->addWidget(text, 1);
      renderedLayout->addLayout(previewSection, 1);
    }
    l->addWidget(renderedPreviewBox);
  }

  // Section 3: Export
  auto exportHairline = new Hairline(w);
  exportHairline->setObjectName("themeExportHairline");
  l->addWidget(exportHairline);
  auto exportSection = new QVBoxLayout;
  exportSection->setContentsMargins(0, 0, 0, 0);
  exportSection->setSpacing(6);

  auto exportHeading = new QLabel("Export", w);
  QFont exportHeadingFont = exportHeading->font();
  exportHeadingFont.setWeight(QFont::DemiBold);
  exportHeading->setFont(exportHeadingFont);
  exportSection->addWidget(exportHeading);

  auto exportDirRow = new QHBoxLayout;
  exportDirRow->setContentsMargins(0, 0, 0, 0);
  exportDirRow->setSpacing(6);
  auto dir =
      line("Export directory", themeDraft.value("directory").toString(), w);
  dir->setPlaceholderText(QString::fromUtf8("Export folder path\u2026"));
  dir->setMinimumWidth(80);
  dir->setEnabled(hasProject);
  connect(dir, &QLineEdit::textChanged, this,
          [this](const QString &value) { themeDraft["directory"] = value; });
  exportDirRow->addWidget(dir, 1);
  auto chooseBtn = button(QString::fromUtf8("Browse\u2026"), w, [this, dir] {
    auto path =
        QFileDialog::getExistingDirectory(this, "Export theme", dir->text());
    if (!path.isEmpty())
      dir->setText(path);
  });
  chooseBtn->setAccessibleName("Choose directory");
  chooseBtn->setIcon(
      themedIcon({"folder-open-symbolic", "folder-open"}, QStyle::SP_DirOpenIcon));
  chooseBtn->setIconSize(QSize(16, 16));
  chooseBtn->setEnabled(hasProject);
  exportDirRow->addWidget(chooseBtn);
  exportSection->addLayout(exportDirRow);

  auto exportActionRow = new QHBoxLayout;
  exportActionRow->setContentsMargins(0, 0, 0, 0);
  exportActionRow->setSpacing(6);
  auto overwrite = new QCheckBox("Overwrite existing files", w);
  overwrite->setChecked(themeDraft.value("overwrite").toBool());
  overwrite->setEnabled(hasProject);
  connect(overwrite, &QCheckBox::toggled, this,
          [this](bool value) { themeDraft["overwrite"] = value; });
  exportActionRow->addWidget(overwrite, 0, Qt::AlignLeft | Qt::AlignVCenter);
  exportActionRow->addStretch(1);

  auto exportBtn = new PrimaryActionButton("Export", w);
  exportBtn->setAccessibleName("Export");
  exportBtn->setIcon(themedIcon({"document-export-symbolic", "document-export"},
                                QStyle::SP_DialogSaveButton));
  exportBtn->setIconSize(QSize(16, 16));
  exportBtn->setEnabled(hasProject);
  auto updateExportButton = [exportBtn, hasProject](const QString &value) {
    const bool hasDir = !value.trimmed().isEmpty();
    exportBtn->setText(hasDir ? QStringLiteral("Export")
                              : QString::fromUtf8("Export\u2026"));
    exportBtn->setPrimary(hasProject && hasDir);
    exportBtn->setToolTip(
        !hasProject ? QStringLiteral("Open or create a theme project to export")
        : hasDir    ? QStringLiteral("Export theme files to the selected folder")
                    : QStringLiteral("Choose a folder and export theme files"));
  };
  updateExportButton(dir->text());
  connect(dir, &QLineEdit::textChanged, exportBtn, updateExportButton);
  connect(exportBtn, &QPushButton::clicked, this, [this, dir, overwrite] {
    QString targetDir = dir->text().trimmed();
    if (targetDir.isEmpty()) {
      targetDir =
          QFileDialog::getExistingDirectory(this, "Export theme", dir->text());
      if (targetDir.isEmpty())
        return;
      dir->setText(targetDir);
    }
    send({{"op", "theme_export"},
          {"directory", dir->text()},
          {"overwrite", overwrite->isChecked()}});
  });
  exportActionRow->addWidget(exportBtn, 0, Qt::AlignRight | Qt::AlignVCenter);
  exportSection->addLayout(exportActionRow);
  l->addLayout(exportSection);

  auto error = new QLabel(str(t, "error"), w);
  error->setWordWrap(true);
  error->setVisible(!error->text().isEmpty());
  l->addWidget(error);
  l->addStretch();

  auto controlsScroll = scroll(w);
  controlsScroll->setObjectName("themeInspector");
  controlsScroll->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
  controlsScroll->setMinimumHeight(48);
  connect(previewToggleAction, &QAction::toggled, this,
          [this, previewContainer, controlsScroll](bool visible) {
            compactThemePreview = visible;
            previewContainer->setVisible(visible);
            if (visible) {
              if (auto *sa = qobject_cast<QScrollArea *>(controlsScroll)) {
                if (auto *vbar = sa->verticalScrollBar())
                  vbar->setValue(0);
              }
            }
          });
  panes->addWidget(previewContainer);
  panes->addWidget(controlsScroll);
  connect(panes, &QSplitter::splitterMoved, this, [this, panes] {
    if (panes->orientation() == Qt::Horizontal && panes->count() == 2) {
      panes->pendingSizes.clear();
      const auto sizes = panes->sizes();
      settings.setValue("board/themeSplitter", QVariantList{sizes[0], sizes[1]});
    }
  });
  outerLayout->addWidget(panes, 1);
  return outer;
}

void Frontend::renderConfig() {
  auto formatWrappableStatus = [](const QString &raw) {
    QString out;
    out.reserve(raw.size() + 16);
    for (int i = 0; i < raw.size(); ++i) {
      const QChar ch = raw[i];
      out.append(ch);
      if ((ch == QLatin1Char('.') || ch == QLatin1Char('/')) &&
          i + 1 < raw.size() && !raw[i + 1].isSpace())
        out.append(QChar(0x200B));
    }
    return out;
  };
  auto updateShortcutDot = [this](const QString &statusText) {
    if (!shortcutDot)
      return;
    const bool hasFailure =
        statusText.contains("unavailable", Qt::CaseInsensitive) ||
        statusText.contains("failed", Qt::CaseInsensitive) ||
        statusText.contains("error", Qt::CaseInsensitive) ||
        statusText.contains("fallback", Qt::CaseInsensitive) ||
        statusText.contains("unable", Qt::CaseInsensitive) ||
        statusText.contains("configure", Qt::CaseInsensitive) ||
        statusText.contains("not running", Qt::CaseInsensitive) ||
        statusText.contains("disconnected", Qt::CaseInsensitive);
    const bool hasActive =
        statusText.contains("ready", Qt::CaseInsensitive) ||
        statusText.contains("GlobalShortcuts", Qt::CaseInsensitive) ||
        statusText.contains("bindings:", Qt::CaseInsensitive);
    if (statusText.contains("Checking", Qt::CaseInsensitive))
      shortcutDot->setColor(QColor(0xF6, 0x74, 0x00));
    else if (hasActive && !hasFailure)
      shortcutDot->setColor(QColor(0x27, 0xAE, 0x60));
    else
      shortcutDot->setColor(StatusDot::neutralColor(palette()));
  };
  auto updateConfigStatusIcon = [this](bool isError) {
    if (!configStatusIcon)
      return;
    QIcon icon = isError
                     ? themedIcon({"dialog-error", "dialog-error-symbolic",
                                   "data-error"},
                                  QStyle::SP_MessageBoxCritical)
                     : themedIcon({"dialog-information-symbolic",
                                   "dialog-information"},
                                  QStyle::SP_MessageBoxInformation);
    configStatusIcon->setPixmap(icon.pixmap(QSize(16, 16), devicePixelRatioF()));
  };
  auto applyLimitErrorStyle = [this](bool invalid) {
    if (configLimit)
      configLimit->setInvalid(invalid);
  };
  const bool hasLoadError = !state.value("load_error").isNull();
  const bool statusIsError = hasLoadError || !str(state, "error").isEmpty();
  if (configSave) {
    limitInvalid = !limitErrorDismissed &&
                   str(state, "validation_field") == "history_limit";
    configSave->setEnabled(!hasLoadError &&
                           !state.value("busy").toBool() && !limitInvalid);
    configRebind->setEnabled(!hasLoadError && !state.value("busy").toBool());
    const QString statusMsg =
        str(state, "validation_field").isEmpty() ? message() : QString();
    configStatus->setText(statusMsg);
    configStatus->setToolTip(str(state, "message_detail"));
    configStatus->setAccessibleDescription(str(state, "message_detail"));
    updateConfigStatusIcon(statusIsError);
    if (configStatusBox)
      configStatusBox->setVisible(!statusMsg.isEmpty());
    const QString scStatus = str(state, "shortcut_status");
    shortcutStatus->setText(formatWrappableStatus(scStatus));
    shortcutStatus->setToolTip(str(state, "shortcut_status_detail"));
    shortcutStatus->setAccessibleDescription(
        str(state, "shortcut_status_detail"));
    if (shortcutDot)
      shortcutDot->setToolTip(str(state, "shortcut_status_detail"));
    updateShortcutDot(scStatus);
    if (shortcutStatusBox)
      shortcutStatusBox->setVisible(!scStatus.isEmpty());
    limitError->setText(str(state, "message"));
    applyLimitErrorStyle(limitInvalid);
    if (limitErrorBox)
      limitErrorBox->setVisible(limitInvalid);
    return;
  }
  if (!configInitialized && state.value("config").isObject()) {
    configDraft = state.value("config").toObject();
    configInitialized = true;
  }

  auto configRoot = new QWidget;
  auto rootLayout = new QVBoxLayout(configRoot);
  rootLayout->setContentsMargins(0, 0, 0, 0);
  rootLayout->setSpacing(0);

  auto w = new QWidget;
  auto l = new QVBoxLayout(w);
  l->setContentsMargins(16, 10, 16, 10);
  l->setSpacing(8);

  const QFontMetrics fm = w->fontMetrics();
  int labelW = 0;
  for (const auto &text : {QStringLiteral("Stored history limit:"),
                           QStringLiteral("Displayed history limit:"),
                           QStringLiteral("Popup grid:"),
                           QStringLiteral("Picker zoom:"),
                           QStringLiteral("Clipboard format:"),
                           QStringLiteral("Pick shortcut:"),
                           QStringLiteral("History shortcut:")})
    labelW = qMax(labelW, fm.horizontalAdvance(text));
  labelW += 6;

  auto applyMuted = [](QLabel *label, int alpha = 155) {
    QPalette pal = label->palette();
    const QColor muted = mutedColor(pal, alpha);
    pal.setColor(QPalette::WindowText, muted);
    pal.setColor(QPalette::Text, muted);
    label->setPalette(pal);
  };
  auto makeLabelSpacer = [labelW](QWidget *parent) {
    auto *spacer = new QWidget(parent);
    spacer->setFixedSize(labelW, 1);
    spacer->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    return spacer;
  };
  auto makeSectionHeading = [w](const QString &title) {
    auto *heading = new QLabel(title, w);
    QFont hf = heading->font();
    hf.setWeight(QFont::Bold);
    if (hf.pointSizeF() > 0)
      hf.setPointSizeF(hf.pointSizeF() + 1.0);
    else if (hf.pixelSize() > 0)
      hf.setPixelSize(hf.pixelSize() + 1);
    heading->setFont(hf);
    return heading;
  };
  auto addFormRow = [w, labelW, makeLabelSpacer](
                        QVBoxLayout *section, const QString &labelText,
                        QWidget *field, QWidget *buddy = nullptr,
                        bool stretchField = false, int alignLabelHeight = 0) {
    auto *row = new QHBoxLayout;
    row->setContentsMargins(0, 0, 0, 0);
    row->setSpacing(10);
    if (!labelText.isEmpty()) {
      auto *lbl = new QLabel(labelText, w);
      lbl->setFixedWidth(labelW);
      lbl->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
      lbl->setBuddy(buddy ? buddy : field);
      if (alignLabelHeight > 0) {
        lbl->setFixedHeight(alignLabelHeight);
        row->addWidget(lbl, 0, Qt::AlignTop);
      } else {
        row->addWidget(lbl, 0, Qt::AlignVCenter);
      }
    } else {
      row->addWidget(makeLabelSpacer(w), 0, Qt::AlignVCenter);
    }
    if (stretchField) {
      row->addWidget(field, 1);
    } else {
      row->addWidget(field, 0, Qt::AlignLeft | Qt::AlignVCenter);
      row->addStretch(1);
    }
    section->addLayout(row);
  };

  // Section 1: History
  auto historySection = new QVBoxLayout;
  historySection->setContentsMargins(0, 0, 0, 0);
  historySection->setSpacing(5);
  historySection->addWidget(makeSectionHeading("History"));

  auto limitFieldBox = new QWidget(w);
  auto limitFieldLayout = new QVBoxLayout(limitFieldBox);
  limitFieldLayout->setContentsMargins(0, 0, 0, 0);
  limitFieldLayout->setSpacing(4);

  auto limit =
      new HistoryLimitSpinBox(str(configDraft, "history_limit"), limitFieldBox);
  limit->setFixedWidth(140);
  const int limitFieldH = limit->sizeHint().height();
  limit->setMinimumHeight(limitFieldH);
  configLimit = limit;
  limitFieldLayout->addWidget(limit, 0, Qt::AlignLeft);

  auto limitBanner =
      new InlineBanner(QColor(218, 68, 83), 24, 145, limitFieldBox);
  auto bannerLayout = new QHBoxLayout(limitBanner);
  bannerLayout->setContentsMargins(8, 5, 8, 5);
  bannerLayout->setSpacing(7);
  auto errIconLabel = new QLabel(limitBanner);
  QIcon errIcon = themedIcon(
      {"dialog-error", "dialog-error-symbolic", "data-error"},
      QStyle::SP_MessageBoxCritical);
  errIconLabel->setPixmap(errIcon.pixmap(QSize(16, 16), devicePixelRatioF()));
  errIconLabel->setFixedSize(16, 16);
  errIconLabel->setAlignment(Qt::AlignCenter);
  bannerLayout->addWidget(errIconLabel, 0, Qt::AlignLeft | Qt::AlignVCenter);
  limitInvalid = !limitErrorDismissed &&
                 str(state, "validation_field") == "history_limit";
  const QString initialLimitMsg =
      limitInvalid
          ? str(state, "message")
          : QStringLiteral("Stored history limit must be greater than zero.");
  limitError = new QLabel(initialLimitMsg, limitBanner);
  limitError->setWordWrap(true);
  limitError->setTextFormat(Qt::PlainText);
  if (palette().color(QPalette::Window).lightness() >= 128) {
    QPalette errPal = limitError->palette();
    errPal.setColor(QPalette::WindowText, QColor(0xA6, 0x1D, 0x2B));
    errPal.setColor(QPalette::Text, QColor(0xA6, 0x1D, 0x2B));
    limitError->setPalette(errPal);
  }
  bannerLayout->addWidget(limitError, 1);
  limitErrorBox = limitBanner;
  limitErrorBox->setVisible(limitInvalid);
  applyLimitErrorStyle(limitInvalid);
  limitFieldLayout->addWidget(limitErrorBox);

  connect(limit->lineEdit(), &QLineEdit::textChanged, this,
          [this](const QString &v) {
            configDraft["history_limit"] = v;
            if (limitInvalid) {
              limitInvalid = false;
              limitErrorDismissed = true;
              if (configLimit)
                configLimit->setInvalid(false);
              if (limitErrorBox)
                limitErrorBox->setVisible(false);
              if (configSave)
                configSave->setEnabled(state.value("load_error").isNull() &&
                                       !state.value("busy").toBool());
            }
          });
  addFormRow(historySection, "Stored history limit:", limitFieldBox, limit,
             true, limitFieldH);

  auto displayLimit =
      spin("Displayed history limit",
           configDraft.value("history_display_limit").toInt(), 1, 10000, w);
  displayLimit->setFixedWidth(140);
  connect(displayLimit, &QSpinBox::valueChanged, this, [this](int v) {
    configDraft["history_display_limit"] = v;
  });
  addFormRow(historySection, "Displayed history limit:", displayLimit);

  auto gridRow = new QWidget(w);
  auto gridLayout = new QHBoxLayout(gridRow);
  gridLayout->setContentsMargins(0, 0, 0, 0);
  gridLayout->setSpacing(6);
  auto cols = spin("History columns",
                   configDraft.value("history_columns").toInt(), 1, 12, gridRow);
  cols->setFixedWidth(60);
  connect(cols, &QSpinBox::valueChanged, this,
          [this](int v) { configDraft["history_columns"] = v; });
  auto timesLabel = new QLabel(QString::fromUtf8("\u00D7"), gridRow);
  applyMuted(timesLabel, 160);
  auto rows = spin("History rows", configDraft.value("history_rows").toInt(), 1,
                   12, gridRow);
  rows->setFixedWidth(60);
  connect(rows, &QSpinBox::valueChanged, this,
          [this](int v) { configDraft["history_rows"] = v; });
  auto gridHint =
      new QLabel(QString::fromUtf8("(columns \u00D7 rows)"), gridRow);
  applyMuted(gridHint, 145);
  gridLayout->addWidget(cols);
  gridLayout->addWidget(timesLabel);
  gridLayout->addWidget(rows);
  gridLayout->addWidget(gridHint);
  addFormRow(historySection, "Popup grid:", gridRow, cols);

  l->addLayout(historySection);
  l->addWidget(new Hairline(w));

  // Section 2: Picker & Clipboard
  auto pickerSection = new QVBoxLayout;
  pickerSection->setContentsMargins(0, 0, 0, 0);
  pickerSection->setSpacing(5);
  pickerSection->addWidget(makeSectionHeading("Picker & Clipboard"));

  auto zoom = spin("Picker zoom", configDraft.value("picker_zoom").toInt(), 1,
                   32, w);
  zoom->setSuffix(QString::fromUtf8("\u00D7"));
  zoom->setFixedWidth(140);
  connect(zoom, &QSpinBox::valueChanged, this,
          [this](int v) { configDraft["picker_zoom"] = v; });
  addFormRow(pickerSection, "Picker zoom:", zoom);

  auto format = new QComboBox(w);
  format->setAccessibleName("Clipboard format");
  format->setFixedWidth(140);
  for (auto value : {"hex", "hex-rgba", "rgb", "rgba", "hsl", "hsv", "cmyk"})
    format->addItem(QString(value).toUpper(), value);
  format->setCurrentIndex(
      qMax(0, format->findData(str(configDraft, "clipboard_format"))));
  connect(format, &QComboBox::currentIndexChanged, this, [this, format] {
    configDraft["clipboard_format"] = format->currentData().toString();
  });
  addFormRow(pickerSection, "Clipboard format:", format);

  auto addCheck = [&](QVBoxLayout *section, const char *key,
                      const QString &label) {
    auto c = new QCheckBox(label, w);
    c->setChecked(configDraft.value(key).toBool());
    connect(c, &QCheckBox::toggled, this,
            [this, key](bool v) { configDraft[key] = v; });
    addFormRow(section, QString(), c);
  };
  addCheck(pickerSection, "auto_copy", "Automatically copy picked colors");
  addCheck(pickerSection, "hide_board_while_picking",
           "Hide board while picking");

  l->addLayout(pickerSection);
  l->addWidget(new Hairline(w));

  // Section 3: Shortcuts & Startup
  auto shortcutSection = new QVBoxLayout;
  shortcutSection->setContentsMargins(0, 0, 0, 0);
  shortcutSection->setSpacing(5);
  shortcutSection->addWidget(makeSectionHeading("Shortcuts & Startup"));

  auto addShortcutField = [&](const char *key, const QString &name,
                              const QString &label) {
    auto e = line(name, str(configDraft, key), w);
    e->setFixedWidth(180);
    connect(e, &QLineEdit::textChanged, this,
            [this, key](const QString &v) { configDraft[key] = v; });
    addFormRow(shortcutSection, label, e);
  };
  addShortcutField("pick_shortcut", "Pick shortcut", "Pick shortcut:");
  addShortcutField("history_shortcut", "History shortcut", "History shortcut:");

  configRebind =
      new QPushButton(themedIcon({"configure-shortcuts-symbolic",
                                  "configure-shortcuts", "view-refresh-symbolic",
                                  "view-refresh"},
                                 QStyle::SP_BrowserReload),
                      "Rebind shortcuts", w);
  configRebind->setAccessibleName("Rebind shortcuts");
  configRebind->setIconSize(QSize(16, 16));
  configRebind->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
  configRebind->setEnabled(!hasLoadError && !state.value("busy").toBool());
  connect(configRebind, &QPushButton::clicked, this,
          [this] { send({{"op", "rebind"}}); });
  addFormRow(shortcutSection, QString(), configRebind, configRebind);

  auto scStatusRow = new QWidget(w);
  scStatusRow->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
  auto scStatusRowLayout = new QHBoxLayout(scStatusRow);
  scStatusRowLayout->setContentsMargins(0, 0, 0, 0);
  scStatusRowLayout->setSpacing(10);
  scStatusRowLayout->addWidget(makeLabelSpacer(scStatusRow), 0, Qt::AlignTop);

  auto scStatusInner = new QHBoxLayout;
  scStatusInner->setContentsMargins(0, 0, 0, 0);
  scStatusInner->setSpacing(6);
  auto dotWrap = new QWidget(scStatusRow);
  auto dotWrapLayout = new QVBoxLayout(dotWrap);
  dotWrapLayout->setContentsMargins(0, qMax(0, (fm.height() - 8) / 2), 0, 0);
  dotWrapLayout->setSpacing(0);
  shortcutDot = new StatusDot(dotWrap);
  shortcutDot->setToolTip(str(state, "shortcut_status_detail"));
  dotWrapLayout->addWidget(shortcutDot, 0, Qt::AlignTop | Qt::AlignLeft);
  scStatusInner->addWidget(dotWrap, 0, Qt::AlignTop | Qt::AlignLeft);

  const QString initialScStatus = str(state, "shortcut_status");
  shortcutStatus =
      new QLabel(formatWrappableStatus(initialScStatus), scStatusRow);
  shortcutStatus->setToolTip(str(state, "shortcut_status_detail"));
  shortcutStatus->setAccessibleDescription(
      str(state, "shortcut_status_detail"));
  shortcutStatus->setWordWrap(true);
  shortcutStatus->setTextFormat(Qt::PlainText);
  shortcutStatus->setMinimumWidth(0);
  shortcutStatus->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
  applyMuted(shortcutStatus, 170);
  scStatusInner->addWidget(shortcutStatus, 1);
  scStatusRowLayout->addLayout(scStatusInner, 1);

  shortcutStatusBox = scStatusRow;
  updateShortcutDot(initialScStatus);
  shortcutStatusBox->setVisible(!initialScStatus.isEmpty());
  shortcutSection->addWidget(shortcutStatusBox);
  addCheck(shortcutSection, "autostart", "Start automatically");

  l->addLayout(shortcutSection);
  l->addStretch(1);

  configScroll = static_cast<QScrollArea *>(scroll(w));
  rootLayout->addWidget(configScroll, 1);

  // Optional subtle inline feedback banner above the footer hairline
  configStatusBox = new QWidget(configRoot);
  auto statusBoxLayout = new QVBoxLayout(configStatusBox);
  statusBoxLayout->setContentsMargins(0, 0, 0, 0);
  statusBoxLayout->setSpacing(0);
  statusBoxLayout->addWidget(new Hairline(configStatusBox));
  auto statusInner = new QHBoxLayout;
  statusInner->setContentsMargins(14, 7, 14, 7);
  statusInner->setSpacing(8);
  configStatusIcon = new QLabel(configStatusBox);
  configStatusIcon->setFixedSize(16, 16);
  updateConfigStatusIcon(statusIsError);
  statusInner->addWidget(configStatusIcon, 0, Qt::AlignLeft | Qt::AlignVCenter);
  const QString initialStatus =
      str(state, "validation_field").isEmpty() ? message() : QString();
  configStatus = new QLabel(initialStatus, configStatusBox);
  configStatus->setToolTip(str(state, "message_detail"));
  configStatus->setAccessibleDescription(str(state, "message_detail"));
  configStatus->setWordWrap(true);
  configStatus->setTextFormat(Qt::PlainText);
  statusInner->addWidget(configStatus, 1);
  statusBoxLayout->addLayout(statusInner);
  configStatusBox->setVisible(!initialStatus.isEmpty());
  rootLayout->addWidget(configStatusBox);

  // Pinned edge-to-edge KDE dialog footer bar
  rootLayout->addWidget(new Hairline(configRoot));
  auto footer = new QWidget(configRoot);
  auto footerLayout = new QHBoxLayout(footer);
  footerLayout->setContentsMargins(14, 10, 14, 10);
  footerLayout->setSpacing(8);
  footerLayout->addStretch(1);

  auto saveBtn = new QPushButton(
      themedIcon({"dialog-ok-apply-symbolic", "dialog-ok-apply"},
                 QStyle::SP_DialogApplyButton),
      "Save", footer);
  saveBtn->setAccessibleName("Save settings");
  saveBtn->setIconSize(QSize(16, 16));
  saveBtn->setEnabled(!hasLoadError && !state.value("busy").toBool() &&
                      !limitInvalid);
  connect(saveBtn, &QPushButton::clicked, this, [this] {
    limitErrorDismissed = false;
    send({{"op", "save"}, {"config", configDraft}}, true);
    if (str(state, "validation_field") == "history_limit") {
      configLimit->setFocus();
      configScroll->ensureWidgetVisible(configLimit);
    }
  });
  configSave = saveBtn;
  footerLayout->addWidget(configSave);

  auto closeButton =
      new QPushButton(themedIcon({"dialog-close-symbolic", "dialog-close",
                                  "window-close-symbolic"},
                                 QStyle::SP_DialogCloseButton),
                      "Close", footer);
  closeButton->setAccessibleName("Close");
  closeButton->setIconSize(QSize(16, 16));
  connect(closeButton, &QPushButton::clicked, this, [this] { close(); });
  footerLayout->addWidget(closeButton);

  rootLayout->addWidget(footer);
  replace(configRoot);
}
void Frontend::renderHistory() {
  historyButtons.clear();
  auto popup = new PopupFrame;
  auto popupLayout = new QVBoxLayout(popup);
  popupLayout->setContentsMargins(1, 1, 1, 1);
  popupLayout->setSpacing(0);

  auto header = new QWidget(popup);
  auto headerLayout = new QHBoxLayout(header);
  headerLayout->setContentsMargins(10, 7, 10, 7);
  headerLayout->setSpacing(10);
  auto title = new QLabel("Color history", header);
  QFont titleFont = title->font();
  titleFont.setWeight(QFont::DemiBold);
  title->setFont(titleFont);
  headerLayout->addWidget(title);
  headerLayout->addStretch(1);
  auto hint = new QLabel(
      QString::fromUtf8("Click to copy \u00B7 Esc to close"), header);
  QFont hintFont = hint->font();
  if (hintFont.pointSizeF() > 0)
    hintFont.setPointSizeF(qMax(8.0, hintFont.pointSizeF() - 1.0));
  else if (hintFont.pixelSize() > 0)
    hintFont.setPixelSize(qMax(9, hintFont.pixelSize() - 1));
  hint->setFont(hintFont);
  QPalette hintPal = hint->palette();
  const QColor hintFg = mutedColor(hintPal, 145);
  hintPal.setColor(QPalette::WindowText, hintFg);
  hintPal.setColor(QPalette::Text, hintFg);
  hint->setPalette(hintPal);
  headerLayout->addWidget(hint);
  popupLayout->addWidget(header);
  popupLayout->addWidget(new Hairline(popup));

  auto w = new QWidget;
  auto l = new QVBoxLayout(w);
  const auto entries = array(state, "entries");
  const int columns = qMax(1, state.value("columns").toInt(4));
  const int rows = qMax(1, state.value("rows").toInt(3));
  if (entries.isEmpty()) {
    l->setContentsMargins(16, 18, 16, 18);
    l->setSpacing(6);
    l->addStretch(1);
    QPixmap emptyIcon = makeEmptyHistoryIcon(22, 115);
    if (!emptyIcon.isNull()) {
      auto iconLabel = new QLabel(w);
      iconLabel->setPixmap(emptyIcon);
      iconLabel->setAlignment(Qt::AlignCenter);
      l->addWidget(iconLabel, 0, Qt::AlignHCenter);
    }
    auto emptyLabel = new QLabel("No picked colors yet", w);
    emptyLabel->setAlignment(Qt::AlignCenter);
    QPalette emptyPal = emptyLabel->palette();
    const QColor emptyFg = mutedColor(emptyPal, 145);
    emptyPal.setColor(QPalette::WindowText, emptyFg);
    emptyPal.setColor(QPalette::Text, emptyFg);
    emptyLabel->setPalette(emptyPal);
    l->addWidget(emptyLabel);
    l->addStretch(1);
  } else {
    l->setContentsMargins(0, 0, 0, 0);
    l->setSpacing(0);
    auto grid = new QGridLayout;
    grid->setContentsMargins(10, 10, 10, 10);
    grid->setSpacing(6);
    for (int col = 0; col < columns; ++col)
      grid->setColumnStretch(col, 1);
    int index = 0;
    for (auto value : entries) {
      auto o = value.toObject();
      auto b = new Swatch(color(str(o, "color")), str(o, "text"), w);
      b->setTileMode(true, false);
      b->setMinimumSize(104, 42);
      b->setFixedHeight(42);
      b->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
      b->setToolTip(str(o, "detail"));
      b->installEventFilter(this);
      historyButtons.append(b);
      auto id = o.value("id");
      connect(b, &QPushButton::clicked, this, [this, id] {
        auto result = request({{"op", "select"}, {"id", id}});
        if (result.isObject()) {
          auto o = result.toObject();
          if (o.value("close").toBool())
            close();
          else
            updateState(o);
        }
      });
      grid->addWidget(b, index / columns, index % columns);
      ++index;
    }
    l->addLayout(grid);
    l->addStretch(1);
  }

  auto sc = static_cast<QScrollArea *>(scroll(w));
  historyScroll = sc;
  const int actualRows =
      entries.isEmpty() ? 0 : (entries.size() + columns - 1) / columns;
  const int visibleRows = qMin(actualRows, rows);
  const int gridVerticalPad = actualRows > rows ? 16 : 20;
  const int naturalGridH =
      visibleRows * 42 + qMax(0, visibleRows - 1) * 6 + gridVerticalPad;
  const bool needsVScroll =
      !entries.isEmpty() && (actualRows > rows || naturalGridH > 650);
  const int contentW = columns * 104 + qMax(0, columns - 1) * 6 + 20;
  const int vbarW = needsVScroll
                        ? sc->style()->pixelMetric(QStyle::PM_ScrollBarExtent,
                                                   nullptr, sc)
                        : 0;
  const int headerMinW = header->sizeHint().width();
  const int naturalW = qMax(contentW + vbarW, headerMinW);
  const int targetW = qMin(900, naturalW);
  const bool needsHScroll = naturalW > 900;
  const int hbarH = needsHScroll
                        ? sc->style()->pixelMetric(QStyle::PM_ScrollBarExtent,
                                                   nullptr, sc)
                        : 0;
  const int targetH =
      entries.isEmpty() ? 110 : qMin(650, naturalGridH + hbarH);
  sc->setHorizontalScrollBarPolicy(needsHScroll ? Qt::ScrollBarAsNeeded
                                                : Qt::ScrollBarAlwaysOff);
  sc->setVerticalScrollBarPolicy(needsVScroll ? Qt::ScrollBarAsNeeded
                                              : Qt::ScrollBarAlwaysOff);
  sc->setFixedSize(targetW, targetH);
  popupLayout->addWidget(sc, 1);

  const QString errText = str(state, "error");
  if (!errText.isEmpty()) {
    popupLayout->addWidget(new Hairline(popup));
    auto errContainer = new QWidget(popup);
    errContainer->setObjectName("historyErrorBanner");
    auto errLayout = new QHBoxLayout(errContainer);
    errLayout->setContentsMargins(10, 7, 10, 7);
    errLayout->setSpacing(7);
    auto errIcon = new QLabel(errContainer);
    errIcon->setPixmap(
        themedIcon({"dialog-error", "dialog-error-symbolic", "data-error"},
                   QStyle::SP_MessageBoxCritical)
            .pixmap(QSize(16, 16), devicePixelRatioF()));
    errIcon->setFixedSize(16, 16);
    errLayout->addWidget(errIcon, 0, Qt::AlignLeft | Qt::AlignVCenter);
    auto error = new QLabel(errText, errContainer);
    error->setWordWrap(true);
    error->setTextFormat(Qt::PlainText);
    error->setMinimumWidth(0);
    error->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    if (palette().color(QPalette::Window).lightness() >= 128) {
      QPalette errPal = error->palette();
      errPal.setColor(QPalette::WindowText, QColor(0xA6, 0x1D, 0x2B));
      errPal.setColor(QPalette::Text, QColor(0xA6, 0x1D, 0x2B));
      error->setPalette(errPal);
    }
    errLayout->addWidget(error, 1);
    popupLayout->addWidget(errContainer);
  }

  replace(popup);
  adjustSize();
  if (!historyButtons.isEmpty())
    historyButtons.first()->setFocus();
}
extern "C" int swatch_qt_run(const char *mode, Callback callback,
                             void *context) {
  int argc = 1;
  char name[] = "swatch-board";
  char *argv[] = {name, nullptr};
  QApplication app(argc, argv);
  QCoreApplication::setOrganizationName("apartment38");
  QCoreApplication::setApplicationName("Swatch Board");
  QString m = QString::fromUtf8(mode);
  QGuiApplication::setDesktopFileName("org.apartment38.SwatchBoard." +
                                      (m == "board"    ? QString("Board")
                                       : m == "config" ? QString("Config")
                                                       : QString("History")));
  Frontend window(QString::fromUtf8(mode), callback, context);
  if (QString::fromUtf8(mode) == "history") {
    auto screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
      screen = QGuiApplication::primaryScreen();
    window.adjustSize();
    if (screen) {
      auto area = screen->availableGeometry();
      auto pos = QCursor::pos();
      window.move(
          qBound(area.left(), pos.x(),
                 qMax(area.left(), area.right() - window.width() + 1)),
          qBound(area.top(), pos.y(),
                 qMax(area.top(), area.bottom() - window.height() + 1)));
    }
  }
  window.show();
  window.smoke();
  return app.exec();
}
