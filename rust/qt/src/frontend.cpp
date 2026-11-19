#include <QtCore>
#include <QtGui>
#include <QtWidgets>
#include <functional>
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

class Swatch : public QPushButton {
  QColor c;
  QPoint start;
  int paletteIndex = -1, historyIndex = -1;
  std::function<void(int, QString)> dropped;

public:
  Swatch(const QColor &value, const QString &name, QWidget *parent = nullptr)
      : QPushButton(name, parent), c(value) {
    setAccessibleName(name + " " + hex(c));
    setMinimumSize(64, 30);
    QPixmap pixmap(24, 24);
    pixmap.fill(Qt::transparent);
    QPainter painter(&pixmap);
    painter.fillRect(QRect(1, 1, 22, 22), c);
    painter.setPen(palette().color(QPalette::Text));
    painter.drawRect(1, 1, 21, 21);
    setIcon(QIcon(pixmap));
    setIconSize(QSize(24, 24));
  }
  void paletteDrop(int index, std::function<void(int, QString)> f) {
    paletteIndex = index;
    dropped = f;
    setAcceptDrops(true);
  }
  void historyDrag(int index) { historyIndex = index; }

protected:
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

public:
  PaletteDrop(std::function<void(int, QString)> f, QWidget *parent)
      : QLabel("Drop a history color here to add it", parent), append(f) {
    setAcceptDrops(true);
    setAccessibleName("Drop a history color to add it to the palette");
    setMinimumHeight(sizeHint().height());
  }

protected:
  void dragEnterEvent(QDragEnterEvent *e) override {
    if (e->mimeData()->hasText() &&
        !e->mimeData()->hasFormat("application/x-swatch-board-index"))
      e->acceptProposedAction();
  }
  void dropEvent(QDropEvent *e) override {
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
    QLinearGradient a(0, 0, width(), 0);
    a.setColorAt(0, Qt::white);
    a.setColorAt(1, QColor::fromHsvF(std::fmod(h, 360.0) / 360.0, 1, 1));
    p.fillRect(rect(), a);
    QLinearGradient b(0, 0, 0, height());
    b.setColorAt(0, Qt::transparent);
    b.setColorAt(1, Qt::black);
    p.fillRect(rect(), b);
    p.setPen(Qt::white);
    p.drawEllipse(
        QPoint(qRound(s * (width() - 1) / 100), qRound((100 - v) * (height() - 1) / 100)), 5,
        5);
  }
  void mousePressEvent(QMouseEvent *e) override { choose(e->pos(), false); }
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
    setFixedSize(120, 120);
    setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Preferred);
    setFocusPolicy(Qt::StrongFocus);
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
    const int side = qMax(1, int(r.width()));
    QImage image(side, side, QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::transparent);
    const double radius = side / 2.0;
    for (int y = 0; y < side; ++y) for (int x = 0; x < side; ++x) {
      double dx = x + .5 - radius, dy = y + .5 - radius;
      double sat = std::hypot(dx, dy) / radius;
      if (sat <= 1) {
        double hue = std::atan2(-dy, dx) / (2 * M_PI);
        if (hue < 0) hue += 1;
        image.setPixelColor(x, y, QColor::fromHsvF(hue, sat, 1));
      }
    }
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);
    p.drawImage(r.topLeft(), image);
    const double angle = h * M_PI / 180.0;
    const QPointF marker = r.center() + QPointF(std::cos(angle), -std::sin(angle)) * (s * r.width() / 200.0);
    p.setPen(QPen(Qt::black, 3)); p.drawEllipse(marker, 6, 6);
    p.setPen(QPen(Qt::white, 1.5)); p.drawEllipse(marker, 6, 6);
    if (hasFocus()) { p.setPen(QPen(palette().color(QPalette::Highlight), 2)); p.drawRect(rect().adjusted(1, 1, -2, -2)); }
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

public:
  QRectF targetRect() const {
    double w = scene.value("width").toDouble(1),
           h = scene.value("height").toDouble(1);
    if (w <= 0 || h <= 0)
      return {};
    double scale = qMin(width() / w, height() / h);
    return QRectF((width() - w * scale) / 2, topAligned ? 0 : (height() - h * scale) / 2,
                  w * scale, h * scale);
  }

private:
  QJsonObject hit(QPoint point) const {
    auto r = targetRect();
    if (!r.contains(point))
      return {};
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
        return e;
    }
    return {};
  }

public:
  Scene(QJsonObject data, std::function<void(QJsonObject)> f, QWidget *parent)
      : QWidget(parent), scene(data), send(f) {
    setMinimumSize(140, 80);
    setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    setAcceptDrops(true);
    setAccessibleName("Theme scene preview");
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
  QSize sizeHint() const override { return QSize(300, 180); }
  void setTopAligned(bool aligned) {
    topAligned = aligned;
    update();
  }

protected:
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    auto r = targetRect();
    if (r.isEmpty())
      return;
    p.translate(r.topLeft());
    p.scale(r.width() / scene.value("width").toDouble(1),
            r.height() / scene.value("height").toDouble(1));
    auto elements = array(scene, "elements");
    for (int i = 0; i < elements.size(); ++i) {
      auto e = elements[i].toObject();
      auto a = array(e, "rect");
      if (a.size() != 4)
        continue;
      QRectF box(a[0].toDouble(), a[1].toDouble(), a[2].toDouble(),
                 a[3].toDouble());
      auto fill = color(str(e, "fill"));
      if (fill.isValid())
        p.fillRect(box, fill);
      auto border = color(str(e, "border"));
      if (border.isValid() && border.alpha()) {
        p.setPen(border);
        p.drawRect(box);
      }
      if (!images[i].isNull())
        p.drawImage(box, images[i]);
      auto text = str(e, "text");
      if (!text.isEmpty()) {
        p.setPen(color(str(e, "text_color")));
        QFont font = p.font();
        if (e.value("font_size").toDouble() > 0)
          font.setPixelSize(qMax(1, int(e.value("font_size").toDouble())));
        p.setFont(font);
        p.drawText(box, Qt::AlignVCenter | Qt::AlignLeft | Qt::TextWordWrap,
                   text);
      }
    }
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
    if (e->mimeData()->hasText())
      e->acceptProposedAction();
  }
  void dropEvent(QDropEvent *e) override {
    auto item = hit(e->position().toPoint());
    auto role = str(item, "role"), c = e->mimeData()->text();
    if (!role.isEmpty() && color(c).isValid()) {
      send({{"op", "theme_assign"}, {"role", role}, {"color", c}});
      e->acceptProposedAction();
    }
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
  QLabel *status = nullptr, *connectionStatus = nullptr;
  QAction *pickAction = nullptr;
  QShortcut *undoShortcut = nullptr, *redoShortcut = nullptr;
  QVBoxLayout *root;
  QVBoxLayout *historyLayout = nullptr;
  QWidget *historyBox = nullptr;
  QScrollArea *historyScroll = nullptr;
  QSplitter *splitter = nullptr, *verticalSplitter = nullptr;
  QWidget *pinnedScroll = nullptr, *pinnedCurrent = nullptr;
  QVBoxLayout *boardContent = nullptr;
  QPushButton *configSave = nullptr, *configRebind = nullptr;
  QLabel *configStatus = nullptr, *shortcutStatus = nullptr, *limitError = nullptr;
  QLineEdit *configLimit = nullptr;
  QScrollArea *configScroll = nullptr;
  QList<QPushButton *> historyButtons;
  bool hiddenForPick = false, spectrumDragging = false;
  int mixerFormat = 0;
  QPointer<Spectrum> mixerSpectrum;
  QPointer<MixerWheel> mixerWheel;
  QPointer<QLabel> mixerReadout;
  QJsonObject configDraft, generationDraft, themeDraft;
  bool compactThemePreview = false;
  bool themeProjectVisible = false;
  bool configInitialized = false;
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
  void send(QJsonObject req) {
    auto result = request(req);
    if (result.isObject())
      updateState(result.toObject());
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
  void updateState(const QJsonObject &next) {
    if (next == state)
      return;
    auto previous = state;
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
  bool compactShelf = false;
  int normalPaletteShelfHeight = 110;
  bool shortMixer() const {
    return height() <= qMax(360, fontMetrics().height() * 24);
  }
  void sizePinnedPalette() {
    if (!pinnedScroll || !tabs) return;
    auto page = tabs->widget(2);
    bool compact = tabs->currentIndex() == 2 && page &&
                   page->height() < page->fontMetrics().height() * 22;
    const bool mixerEmptyShelf = tabs->currentIndex() == 0 && shortMixer() &&
                                 array(object(state.value("palette")), "swatches").isEmpty();
    const int shelfThreshold = fontMetrics().height() * 24;
    const bool shrinkShelf = (tabs->currentIndex() == 1 && height() < shelfThreshold) ||
                             mixerEmptyShelf;
    const int dropHeight = fontMetrics().height() + 8;
    if (verticalSplitter && shrinkShelf && !compactShelf)
      normalPaletteShelfHeight = verticalSplitter->sizes().value(1, 110);
    pinnedScroll->setMinimumHeight(mixerEmptyShelf ? dropHeight : 38);
    pinnedScroll->setMaximumHeight(mixerEmptyShelf ? dropHeight :
                                   compact ? page->fontMetrics().height() * 3 : QWIDGETSIZE_MAX);
    if (verticalSplitter && shrinkShelf != compactShelf) {
      verticalSplitter->setSizes({verticalSplitter->height(),
                                  shrinkShelf ? pinnedScroll->minimumHeight()
                                              : normalPaletteShelfHeight});
      compactShelf = shrinkShelf;
    }
    if (compact && verticalSplitter)
      verticalSplitter->setSizes({verticalSplitter->height(), page->fontMetrics().height() * 3});
  }
  void preserveView(QWidget *old, QWidget *replacement) {
    auto oldScroll = qobject_cast<QScrollArea *>(old),
         newScroll = qobject_cast<QScrollArea *>(replacement);
    if (!oldScroll && (old->objectName() == "themePage" || old->objectName() == "mixerPage"))
      oldScroll = old->findChild<QScrollArea *>();
    if (!newScroll && (replacement->objectName() == "themePage" || replacement->objectName() == "mixerPage"))
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
        item->widget()->hide();
        item->widget()->deleteLater();
      }
      delete item;
    }
    root->addWidget(w);
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
    if (iconName != "edit-undo" && iconName != "edit-redo")
      return label;
    auto key = iconName == "edit-undo" ? QKeySequence::Undo : QKeySequence::Redo;
    return label + " (" + QKeySequence(key).toString(QKeySequence::NativeText) + ")";
  }
  QToolButton *toolButton(const QString &label, const QString &iconName,
                          QWidget *parent, const std::function<void()> &action) {
    auto b = new QToolButton(parent);
    b->setText(label);
    b->setAccessibleName(label);
    b->setToolTip(actionHint(label, iconName));
    b->setProperty("boardAction", true);
    b->setIcon(QIcon::fromTheme(iconName, style()->standardIcon(QStyle::SP_FileIcon)));
    b->setToolButtonStyle(settings.value("board/toolbarLabels", true).toBool()
                              ? Qt::ToolButtonTextBesideIcon : Qt::ToolButtonIconOnly);
    connect(b, &QToolButton::clicked, this, action);
    return b;
  }
  QString actionIcon(const QString &op) const {
    if (op.contains("undo")) return "edit-undo";
    if (op.contains("redo")) return "edit-redo";
    if (op.contains("copy")) return "edit-copy";
    if (op.contains("save")) return "document-save";
    if (op.contains("new")) return "document-new";
    if (op.contains("mix")) return "applications-graphics";
    return "list-add";
  }
  QWidget *scroll(QWidget *content) {
    auto s = new QScrollArea;
    s->setWidgetResizable(true);
    s->setWidget(content);
    if (content->objectName() == "mixerContent")
      s->viewport()->installEventFilter(this);
    return s;
  }
  QWidget *pinnedPalette() {
    auto p = object(state.value("palette"));
    auto w = new QWidget;
    auto l = new QVBoxLayout(w);
    l->setContentsMargins(0, 0, 0, 0);
    l->setSpacing(0);
    auto swatches = array(p, "swatches");
    if (swatches.isEmpty())
      l->addWidget(new PaletteDrop(
          [this](int i, QString c) {
            if (i >= 0)
              send({{"op", "history_select"}, {"index", i}, {"tab", 1}});
            else {
              send({{"op", "mixer_select"}, {"color", c}});
              send({{"op", "palette_add_current"}});
            }
          }, w));
    auto swatchRow = new QHBoxLayout;
    swatchRow->setSpacing(0);
    l->addLayout(swatchRow);
    int i = 0;
    for (auto value : swatches) {
      auto o = value.toObject();
      int index = i++;
      auto c = color(str(o, "color"));
      auto sw = new Swatch(c, str(o, "name"), w);
      sw->setText(str(o, "color").toUpper());
      sw->setIcon(QIcon());
      sw->setMinimumSize(100, 30);
      sw->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
      auto textColor = c.lightness() < 140 ? "white" : "black";
      sw->setStyleSheet(QString("QPushButton { background-color: %1; color: %2; border: 0; font-weight: bold; } QPushButton:focus { border: 2px solid %2; }")
                            .arg(c.name(), textColor));
      sw->setToolTip(str(o, "name") + " — click to select, drag to reorder; right-click for options");
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
    bool first = !tabs;
    if (first) {
      auto outer = new QWidget;
      auto layout = new QVBoxLayout(outer);
      layout->setContentsMargins(0, 0, 0, 0);
      auto toolbar = new QToolBar(outer);
      toolbar->setMovable(false);
      bool labels = settings.value("board/toolbarLabels", true).toBool();
      toolbar->setToolButtonStyle(labels ? Qt::ToolButtonTextBesideIcon : Qt::ToolButtonIconOnly);
      auto addAction = [this, toolbar](const QString &name, QStyle::StandardPixmap icon,
                                       const std::function<void()> &action) {
        auto a = toolbar->addAction(style()->standardIcon(icon), name);
        connect(a, &QAction::triggered, this, action);
        return a;
      };
      auto nav = new QActionGroup(toolbar);
      nav->setExclusive(true);
      QList<QAction *> pages;
      for (auto pair : QList<QPair<QString, QStyle::StandardPixmap>>{
               {"Mixer", QStyle::SP_ComputerIcon}, {"Palette", QStyle::SP_DirIcon},
               {"Theme", QStyle::SP_DesktopIcon}}) {
        auto action = toolbar->addAction(style()->standardIcon(pair.second), pair.first);
        action->setCheckable(true);
        nav->addAction(action);
        int index = pages.size();
        connect(action, &QAction::triggered, this, [this, index] { tabs->setCurrentIndex(index); });
        pages.append(action);
      }
      toolbar->addSeparator();
      auto pick = addAction("Pick color", QStyle::SP_DialogOpenButton, [this] {
        if (state.value("hide_board_while_picking").toBool()) {
          hiddenForPick = true;
          hide();
          QGuiApplication::sync();
        }
        send({{"op", "pick"}});
      });
      pickAction = pick;
      addAction("Settings", QStyle::SP_FileDialogDetailedView, [this] { send({{"op", "configure"}}); });
      auto toggle = toolbar->addAction(style()->standardIcon(QStyle::SP_FileDialogListView), "History");
      toggle->setCheckable(true);
      toggle->setChecked(settings.value("board/historyVisible", true).toBool());
      auto labelToggle = toolbar->addAction("Show labels");
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
      });
      labelToggle->setIcon(style()->standardIcon(QStyle::SP_FileDialogContentsView));
      layout->addWidget(toolbar);
      splitter = new QSplitter(Qt::Horizontal, outer);
      historyBox = new QGroupBox("History");
      historyLayout = new QVBoxLayout(historyBox);
      historyScroll = new QScrollArea;
      historyScroll->setWidgetResizable(true);
      historyScroll->setWidget(historyBox);
      historyScroll->setVisible(toggle->isChecked());
      connect(toggle, &QAction::toggled, this, [this](bool visible) {
        historyScroll->setVisible(visible);
        settings.setValue("board/historyVisible", visible);
      });
      splitter->addWidget(historyScroll);
      tabs = new QTabWidget(outer);
      tabs->setAccessibleName("Board sections");
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
      tabs->addTab(scroll(palettePage()), "Palette");
      tabs->addTab(themePage(), "Theme");
      tabs->setCurrentIndex(
          qBound(0, settings.value("board/tab", 0).toInt(), 2));
      tabs->tabBar()->hide();
      auto content = new QWidget(splitter);
      boardContent = new QVBoxLayout(content);
      boardContent->setContentsMargins(0, 0, 0, 0);
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
              });
      splitter->addWidget(content);
      verticalSplitter = new QSplitter(Qt::Vertical, outer);
      verticalSplitter->addWidget(splitter);
      pinnedScroll = scroll(pinnedPalette());
      pinnedScroll->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
      pinnedScroll->setMinimumHeight(38);
      verticalSplitter->addWidget(pinnedScroll);
      splitter->setStretchFactor(1, 1);
      verticalSplitter->setStretchFactor(0, 1);
      splitter->restoreState(
          settings.value("board/horizontalSplitter").toByteArray());
      if (!verticalSplitter->restoreState(
              settings.value("board/verticalSplitter").toByteArray()))
        verticalSplitter->setSizes({540, 110});
      layout->addWidget(verticalSplitter, 1);
      status = new QLabel(outer);
      status->setWordWrap(true);
      status->setAccessibleName("Board message");
      connectionStatus = new QLabel(outer);
      connectionStatus->setAccessibleName("Connection status");
      auto statusRow = new QHBoxLayout;
      statusRow->addWidget(status, 1);
      statusRow->addWidget(connectionStatus, 0, Qt::AlignBottom);
      layout->addLayout(statusRow);
      replace(outer);
    } else {
      int current = tabs->currentIndex();
      if (previous.value("mixer") != state.value("mixer")) {
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
        auto replacement = index == 1 ? scroll(page) : page;
        preserveView(old, replacement);
        tabs->removeTab(index);
        tabs->insertTab(index, replacement, title);
        old->deleteLater();
      };
      if (previous.value("mixer") != state.value("mixer")) {
        if (spectrumDragging) {
          auto hsv = array(object(state.value("mixer")), "hsv");
          if (hsv.size() == 3) {
            if (mixerSpectrum) mixerSpectrum->setHsv(hsv[0].toDouble(), hsv[1].toDouble(), hsv[2].toDouble());
            if (mixerWheel) mixerWheel->setHsv(hsv[0].toDouble(), hsv[1].toDouble(), hsv[2].toDouble());
            if (mixerReadout)
              mixerReadout->setText(mixerHsvReadout(hsv[0].toDouble(), hsv[1].toDouble(), hsv[2].toDouble()));
          }
        } else
          refresh(0, mixerPage(), "Mixer");
      }
      if (previous.value("palette") != state.value("palette") ||
          (generationDraft.value("base").toString().isEmpty() &&
           previous.value("mixer") != state.value("mixer")))
        refresh(1, palettePage(), "Palette");
      if (previous.value("theme") != state.value("theme")) {
        Scene *reused = nullptr;
        if (object(previous.value("theme")).value("scene") ==
            object(state.value("theme")).value("scene")) {
          for (auto widget : tabs->widget(2)->findChildren<QWidget *>())
            if ((reused = dynamic_cast<Scene *>(widget)))
              break;
        }
        auto replacement = themePage(reused);
        refresh(2, replacement, "Theme");
        if (object(previous.value("theme")).value("selected") !=
            object(state.value("theme")).value("selected")) {
          QPointer<QWidget> guarded(replacement);
          QTimer::singleShot(0, this, [guarded] {
            if (!guarded) return;
            auto inspector = guarded->findChild<QScrollArea *>("themeInspector");
            auto role = guarded->findChild<QWidget *>("selectedThemeRole");
            if (inspector && role)
              inspector->ensureWidgetVisible(role, 0, 0);
          });
        }
      }
      if (previous.value("palette") != state.value("palette")) {
        auto replacement = scroll(pinnedPalette());
        replacement->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
        replacement->setMinimumHeight(38);
        preserveView(pinnedScroll, replacement);
        auto old = verticalSplitter->replaceWidget(1, replacement);
        pinnedScroll = replacement;
        sizePinnedPalette();
        old->deleteLater();
      }
      tabs->setCurrentIndex(current);
      pinnedCurrent->setVisible(current == 0);
    }
    if (first || previous.value("history") != state.value("history")) {
      while (auto item = historyLayout->takeAt(0)) {
        if (item->widget())
          item->widget()->deleteLater();
        delete item;
      }
      int index = 0;
      for (auto v : array(state, "history")) {
        auto o = v.toObject();
        auto b = new Swatch(color(str(o, "color")), str(o, "text"), historyBox);
        b->setToolTip(str(o, "detail"));
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
    status->setText(message());
    pickAction->setEnabled(state.value("loaded").toBool() &&
                           state.value("connected").toBool() &&
                           !state.value("picking").toBool());
    connectionStatus->setText(state.value("picking").toBool()   ? "Picking…"
                              : !state.value("loaded").toBool() ? "Loading…"
                              : state.value("connected").toBool()
                                  ? "Connected"
                                  : "Disconnected");
  }
  QWidget *currentColor() {
    auto current = new Swatch(color(str(object(state.value("mixer")), "current")), "Current color");
    current->setText("Current color  " + str(object(state.value("mixer")), "current"));
    current->paletteDrop(-1, [this](int, QString c) {
      if (color(c).isValid())
        send({{"op", "mixer_select"}, {"color", c}});
    });
    return current;
  }
  QWidget *mixerPage() {
    auto page = new QWidget;
    page->setObjectName("mixerPage");
    auto layout = new QVBoxLayout(page);
    layout->setContentsMargins(0, 0, 0, 0);
    layout->setSpacing(0);
    layout->addWidget(scroll(mixer()));
    layoutMixerActions(page);
    page->installEventFilter(this);
    return page;
  }
  void layoutMixerActions(QWidget *page) {
    if (!page || page->objectName() != "mixerPage") return;
    auto actions = page->findChild<QToolBar *>("mixerActions");
    auto viewport = page->findChild<QScrollArea *>();
    auto content = viewport->widget();
    const bool pinned = shortMixer();
    auto parent = pinned ? page : content;
    if (actions->parentWidget() == parent) return;
    auto layout = qobject_cast<QBoxLayout *>(parent->layout());
    // Move the same toolbar so each command has one button and one connection.
    layout->insertWidget(pinned ? 0 : 1, actions);
  }
  QString mixerHsvReadout(double hue, double saturation, double value) const {
    return QStringLiteral("H %1°   S %2%   V %3%")
        .arg(locale().toString(hue, 'f', 1), locale().toString(saturation, 'f', 1),
             locale().toString(value, 'f', 1));
  }
  QWidget *mixer() {
    auto m = object(state.value("mixer"));
    auto w = new QWidget;
    w->setObjectName("mixerContent");
    auto l = new QVBoxLayout(w);
    auto hsv = array(m, "hsv"), rgba = array(m, "rgba");
    double h = hsv[0].toDouble(), s = hsv[1].toDouble(), v = hsv[2].toDouble();
    auto columns = new QVBoxLayout;
    columns->setObjectName("mixerColumns");
    auto controls = new QHBoxLayout;
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
    auto wheelArea = new QWidget(w);
    auto wheelLayout = new QVBoxLayout(wheelArea);
    wheelLayout->setContentsMargins(0, 0, 0, 0);
    wheelLayout->setSpacing(0);
    mixerWheel = new MixerWheel(h, s, v, changed, wheelArea);
    mixerReadout = new QLabel(mixerHsvReadout(h, s, v), wheelArea);
    mixerReadout->setObjectName("mixerHsvReadout");
    mixerReadout->setVisible(shortMixer());
    wheelLayout->addWidget(mixerWheel, 0, Qt::AlignHCenter);
    wheelLayout->addWidget(mixerReadout, 0, Qt::AlignHCenter);
    mixerSpectrum = new Spectrum(h, s, v, changed, w);
    controls->addWidget(mixerSpectrum, 0, Qt::AlignTop);
    controls->addWidget(wheelArea, 0, Qt::AlignTop);
    columns->addLayout(controls);
    l->addLayout(columns);
    auto inputs = new QWidget(w);
    auto form = new QFormLayout(inputs);
    form->setContentsMargins(0, 0, 0, 0);
    form->setRowWrapPolicy(QFormLayout::WrapLongRows);
    form->setFieldGrowthPolicy(QFormLayout::FieldsStayAtSizeHint);
    auto hbox = new QWidget(w);
    auto hl = new QHBoxLayout(hbox);
    hl->setContentsMargins(0, 0, 0, 0);
    auto hh = decimal("Hue", h, 0, 360, hbox),
         ss = decimal("Saturation", s, 0, 100, hbox),
         vv = decimal("Value", v, 0, 100, hbox);
    for (auto x : {hh, ss, vv}) {
      x->setDecimals(1);
      x->setValue(x == hh ? h : x == ss ? s : v);
      x->setSingleStep(.1);
      x->setSuffix(x == hh ? QString::fromUtf8("°") : "%");
      x->setMaximumWidth(x->sizeHint().width());
      auto label = new QLabel(x == hh ? "H" : x == ss ? "S" : "V", hbox);
      label->setBuddy(x);
      hl->addWidget(label);
      hl->addWidget(x);
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
    hl->addStretch();
    form->addRow("HSV", hbox);
    auto rw = new QWidget(w);
    auto rl = new QHBoxLayout(rw);
    rl->setContentsMargins(0, 0, 0, 0);
    QList<QSpinBox *> channels;
    const char *names[] = {"Red", "Green", "Blue", "Alpha"};
    for (int i = 0; i < 4; i++) {
      auto x = spin(names[i], rgba[i].toInt(), 0, 255, rw);
      x->setMaximumWidth(x->sizeHint().width());
      channels << x;
      auto label = new QLabel(QString(names[i]).left(1), rw);
      label->setBuddy(x);
      rl->addWidget(label);
      rl->addWidget(x);
    }
    rl->addStretch();
    for (auto x : channels)
      connect(x, &QSpinBox::editingFinished, this, [this, channels] {
        send({{"op", "mixer_rgba"},
              {"r", channels[0]->value()},
              {"g", channels[1]->value()},
              {"b", channels[2]->value()},
              {"a", channels[3]->value()}});
      });
    form->addRow("RGBA", rw);
    auto fields = array(m, "fields");
    auto formatRow = new QWidget(w);
    auto formatLayout = new QHBoxLayout(formatRow);
    formatLayout->setContentsMargins(0, 0, 0, 0);
    auto format = new QComboBox(formatRow);
    format->setAccessibleName("Color format");
    format->addItems({"HEX", "RGB", "RGBA", "HSL", "HSV"});
    format->setCurrentIndex(mixerFormat);
    auto value = line("Color value", fields[mixerFormat].toString(), formatRow);
    value->setMinimumWidth(value->fontMetrics().horizontalAdvance(QString(18, QLatin1Char('0'))) + 20);
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
    formatLayout->addWidget(format);
    formatLayout->addWidget(value);
    formatLayout->addWidget(button("Apply", formatRow, apply));
    auto formatLabel = new QLabel("Format", w);
    formatLabel->setBuddy(format);
    form->addRow(formatLabel, formatRow);
    form->addRow("CMYK", new QLabel(str(m, "cmyk"), w));
    form->addRow("Output", new QLabel(str(m, "output"), w));
    columns->addWidget(inputs, 1, Qt::AlignTop);
    setTabOrder(mixerSpectrum, mixerWheel);
    auto actions = new QToolBar(w);
    actions->setObjectName("mixerActions");
    actions->setMovable(false);
    actions->setProperty("boardAction", true);
    actions->setToolButtonStyle(settings.value("board/toolbarLabels", true).toBool()
                                    ? Qt::ToolButtonTextBesideIcon : Qt::ToolButtonIconOnly);
    for (auto pair : QList<QPair<QString, QString>>{
             {"Copy", "mixer_copy"},
             {"Add to palette", "palette_add_current"},
             {"Undo", "mixer_undo"},
             {"Redo", "mixer_redo"},
             {"Add source", "mixer_source_add"},
             {"Mix", "mixer_mix"}}) {
      const auto iconName = actionIcon(pair.second);
      auto icon = QIcon::fromTheme(iconName, style()->standardIcon(QStyle::SP_FileIcon));
      auto action = actions->addAction(icon, pair.first);
      action->setToolTip(actionHint(pair.first, iconName));
      actions->widgetForAction(action)->setAccessibleName(pair.first);
      connect(action, &QAction::triggered, this, [this, pair] { send({{"op", pair.second}}); });
      if (pair.second == "mixer_undo")
        action->setEnabled(m.value("can_undo").toBool());
      if (pair.second == "mixer_redo")
        action->setEnabled(m.value("can_redo").toBool());
    }
    l->addWidget(actions);
    auto space = new QComboBox(w);
    space->setAccessibleName("Mixing color space");
    space->addItem("sRGB", "srgb");
    space->addItem("OKLCH", "oklch");
    space->setCurrentIndex(qMax(0, space->findData(str(m, "space"))));
    space->setSizeAdjustPolicy(QComboBox::AdjustToContents);
    space->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Fixed);
    connect(space, &QComboBox::currentIndexChanged, this, [this, space] {
      send({{"op", "mixer_space"}, {"space", space->currentData().toString()}});
    });
    l->addWidget(space);
    int i = 0;
    for (auto value : array(m, "sources")) {
      auto o = value.toObject();
      int index = i++;
      auto row = new QWidget(w);
      auto r = new QHBoxLayout(row);
      auto c = line("Source color", str(o, "color"), row);
      c->setMaximumWidth(c->fontMetrics().horizontalAdvance(QString(12, QLatin1Char('M'))) + 20);
      auto weight =
          decimal("Source weight", o.value("weight").toDouble(), 0, 1, row);
      weight->setMaximumWidth(weight->sizeHint().width());
      auto source = new Swatch(color(str(o, "color")), "Source", row);
      source->paletteDrop(-1, [this, index, weight](int, QString c) {
        if (color(c).isValid())
          send({{"op", "mixer_source"},
                {"index", index},
                {"color", c},
                {"weight", weight->value()}});
      });
      r->addWidget(source);
      r->addWidget(c);
      r->addWidget(weight);
      auto apply = [this, index, c, weight] {
        send({{"op", "mixer_source"},
              {"index", index},
              {"color", c->text()},
              {"weight", weight->value()}});
      };
      connect(c, &QLineEdit::editingFinished, this, apply);
      connect(weight, &QDoubleSpinBox::editingFinished, this, apply);
      r->addWidget(button("Remove", row, [this, index] {
        send({{"op", "mixer_source_remove"}, {"index", index}});
      }));
      r->addStretch();
      l->addWidget(row);
    }
    auto error = new QLabel(str(m, "error"), w);
    error->setWordWrap(true);
    l->addWidget(error);
    l->addStretch();
    return w;
  }
  QWidget *palettePage();
  QWidget *themePage(Scene *reuse = nullptr);
  void layoutThemePage(QWidget *page, int width, bool initialize = false);
  void renderConfig();
  void renderHistory();

protected:
  void resizeEvent(QResizeEvent *event) override {
    QWidget::resizeEvent(event);
    sizePinnedPalette();
    if (tabs)
      layoutMixerActions(tabs->widget(0));
  }
  bool eventFilter(QObject *watched, QEvent *event) override {
    if (event->type() == QEvent::Resize) {
      auto page = qobject_cast<QWidget *>(watched);
      if (page && page->objectName() == "mixerPage")
        layoutMixerActions(page);
      auto scroll = page ? qobject_cast<QScrollArea *>(page->parentWidget()) : nullptr;
      if (scroll && page == scroll->viewport() && scroll->widget() &&
          scroll->widget()->objectName() == "mixerContent") {
        auto mixer = scroll->widget();
        auto columns = mixer->findChild<QBoxLayout *>("mixerColumns");
        auto selectors = columns->itemAt(0)->layout();
        auto spectrum = selectors->itemAt(0)->widget();
        auto inputs = columns->itemAt(1)->widget();
        auto wheelArea = selectors->itemAt(1)->widget();
        const auto margins = mixer->layout()->contentsMargins();
        const int widePlane = 208;
        const int wideWheel = 120;
        const bool shortView = shortMixer();
        wheelArea->layout()->itemAt(0)->widget()->setFixedSize(shortView ? 112 : wideWheel,
                                                            shortView ? 112 : wideWheel);
        wheelArea->findChild<QLabel *>("mixerHsvReadout")->setVisible(shortView);
        const int width = static_cast<QResizeEvent *>(event)->size().width();
        const bool wide = width >= margins.left() + margins.right() + widePlane +
                                      wideWheel + inputs->sizeHint().width() +
                                      selectors->spacing() + columns->spacing();
        columns->setDirection(wide ? QBoxLayout::LeftToRight : QBoxLayout::TopToBottom);
        const int side = shortView ? 132 : wide ? widePlane : 156;
        spectrum->setFixedSize(side, side);
      }
      if (page && page->objectName() == "paletteGenerator") {
        auto columns = qobject_cast<QBoxLayout *>(page->layout());
        auto parameters = page->findChild<QWidget *>("paletteParameters", Qt::FindDirectChildrenOnly);
        auto selector = page->findChild<QWidget *>("paletteSelector", Qt::FindDirectChildrenOnly);
        if (columns && parameters && selector) {
          const bool compact = page->width() < 620;
          columns->setDirection(compact ? QBoxLayout::TopToBottom : QBoxLayout::LeftToRight);
          columns->removeWidget(parameters);
          columns->removeWidget(selector);
          columns->addWidget(compact ? selector : parameters, 0, Qt::AlignTop);
          columns->addWidget(compact ? parameters : selector, 0, Qt::AlignTop);
        }
      }
      if (page && page->objectName() == "themePage") {
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
        historyButtons[qBound(0, next, int(historyButtons.size()) - 1)]
            ->setFocus();
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
        for (auto b : tabs->widget(0)->findChildren<QAbstractButton *>())
          if (b->accessibleName() == "Copy" || b->accessibleName() == "Add to palette") {
            check(inMixerViewport(b),
                  b->accessibleName() + " visible in initial Mixer viewport");
          }
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
        for (const auto &name : {QString("Copy"), QString("Add to palette")}) {
          auto matches = mixerPage->findChildren<QAbstractButton *>();
          matches.removeIf([&](QAbstractButton *b) { return b->accessibleName() != name; });
          check(matches.size() == 1 && matches.first()->isVisible() &&
                    mixerPage->rect().contains(QRect(matches.first()->mapTo(mixerPage, QPoint()),
                                                      matches.first()->size())),
                name + " is directly visible in the initial compact Mixer");
        }
        auto emptyDrop = pinnedScroll->findChild<QLabel *>();
        check(emptyDrop && emptyDrop->acceptDrops() && emptyDrop->isVisible() &&
                  pinnedScroll->height() <= fontMetrics().height() + 8,
              "Compact Mixer retains a one-line empty palette drop target");
        check(mixerReadout && mixerReadout->isVisible() && inMixerViewport(mixerReadout) &&
                  mixerReadout->text() == mixerHsvReadout(210.0, 100.0 / 3, 600.0 / 255),
              "Compact Mixer displays current HSV values without scrolling");
        shot("mixer-small");
        for (auto field : mixerScroll->findChildren<QLineEdit *>())
          if (field->accessibleName() == "Color value") {
            mixerScroll->verticalScrollBar()->setValue(
                field->mapTo(mixerScroll->widget(), QPoint()).y());
            shot("mixer-small-format");
            break;
          }
        auto overflow = actions->findChild<QToolButton *>("qt_toolbar_ext_button");
        check(overflow && overflow->isVisible(), "Compact mixer toolbar has overflow");
        if (overflow && overflow->isVisible()) {
          auto menu = overflow->menu();
          check(menu, "Mixer overflow has a menu");
          if (menu) {
            QTimer::singleShot(0, menu, [&] {
              check(menu->isVisible(), "Mixer overflow menu opens");
              QStringList entries;
              for (auto action : menu->actions())
                if (action->isEnabled())
                  entries.append(action->text());
              check(entries.contains("Add source") && entries.contains("Mix"),
                    "Add source and Mix available in compact toolbar overflow");
              check(menu->grab().save(dir + "/mixer-small-overflow.png"),
                    "Screenshot mixer-small-overflow");
              menu->close();
            });
            overflow->showMenu();
          }
        }
        click(mixerPage, "Copy");
        check(QApplication::clipboard()->text() == "#040506", "Pinned Copy copies the Mixer output");
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
          auto paletteScroll = qobject_cast<QScrollArea *>(tabs->widget(1));
          if (wheel) paletteScroll->ensureWidgetVisible(wheel, 0, 0);
          QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
          auto viewport = paletteScroll->viewport();
          check(wheel && viewport->rect().contains(QRect(wheel->mapTo(viewport, QPoint()), wheel->size())) &&
                    viewport->rect().contains(QRect(generate->mapTo(viewport, QPoint()), generate->size())),
                "Compact palette shows complete wheel and Generate together");
          shot("palette-small");
          generate->click();
          check(array(object(state.value("palette")), "swatches").size() == 5,
                "Generate palette at compact size");
        }
        auto captureSavedGenerations = [&](const QString &name) {
          settleLayout();
          auto scroll = qobject_cast<QScrollArea *>(tabs->widget(1));
          auto row = scroll->findChild<QWidget *>("savedGenerationRow");
          auto label = scroll->findChild<QLabel *>("savedGenerationLabel");
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
            e->setText("Qt smoke palette");
            QMetaObject::invokeMethod(e, "editingFinished");
            break;
          }
        click(tabs->widget(1), "Save");
        auto id = str(object(state.value("palette")), "id");
        check(!id.isEmpty(), "Save palette");
        click(tabs->widget(1), "Open");
        check(str(object(state.value("palette")), "id") == id,
              "Reopen palette");
        tabs->setCurrentIndex(2);
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
              name->setText("Smoke theme");
              QMetaObject::invokeMethod(name, "editingFinished");
              check(str(object(object(state.value("theme")).value("project")), "name") == "Smoke theme",
                    "Theme project name updated");
            }
          }
          dialog = tabs->widget(2)->findChild<QDialog *>("themeProjectDialog");
          if (dialog) {
            QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
            check(dialog->grab().save(dir + "/theme-project.png"), "Screenshot theme project");
            dialog->reject();
            check(!dialog->isVisible(), "Theme project closes");
          }
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
          click(tabs->widget(2), "Save");
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
        shot("settings");
        resize(540, 360);
        shot("settings-small");
      } else {
        check(state.contains("entries"), "History entries");
        shot("history");
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
    if (m == "history") {
      setWindowFlags(Qt::Popup);
      setAttribute(Qt::WA_NoMouseReplay);
    } else {
      resize(900, 700);
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
  auto w = new QWidget;
  auto l = new QVBoxLayout(w);
  auto palette = new QGroupBox("Palette", w);
  auto paletteLayout = new QVBoxLayout(palette);
  auto nameRow = new QFormLayout;
  auto name = line("Palette name", str(p, "name"), palette);
  name->setMaximumWidth(260);
  nameRow->addRow("Name", name);
  paletteLayout->addLayout(nameRow);
  connect(name, &QLineEdit::editingFinished, this, [this, name] {
    send({{"op", "palette_name"}, {"name", name->text()}});
  });
  auto actions = new QHBoxLayout;
  for (auto pair :
       QList<QPair<QString, QString>>{{"New draft", "palette_new"},
                                      {"Save", "palette_save"},
                                      {"Duplicate", "palette_duplicate"},
                                      {"Undo", "palette_undo"},
                                      {"Redo", "palette_redo"}}) {
    auto b =
        toolButton(pair.first, actionIcon(pair.second), palette,
                   [this, pair] { send({{"op", pair.second}}); });
    if (pair.second == "palette_undo" || pair.second == "palette_redo") {
      const bool undo = pair.second == "palette_undo";
      b->setProperty("boardAction", false);
      b->setEnabled(p.value(undo ? "can_undo" : "can_redo").toBool());
      b->setToolButtonStyle(Qt::ToolButtonIconOnly);
    }
    actions->addWidget(b);
  }
  actions->addStretch();
  paletteLayout->addLayout(actions);
  l->addWidget(palette);
  auto historyLock = new QCheckBox("Lock history additions", palette);
  historyLock->setChecked(p.value("lock_history").toBool());
  connect(historyLock, &QCheckBox::toggled, this, [this](bool b) {
    send({{"op", "palette_lock_history"}, {"locked", b}});
  });
  paletteLayout->addWidget(historyLock);
  auto library = new QGroupBox("Palette library", w);
  auto lib = new QHBoxLayout(library);
  auto saved =
      combo("Saved palettes", array(p, "saved"), library, str(p, "id"));
  saved->setEditable(true);
  saved->setInsertPolicy(QComboBox::NoInsert);
  saved->lineEdit()->setPlaceholderText("Search saved palettes");
  saved->completer()->setFilterMode(Qt::MatchContains);
  saved->completer()->setCompletionMode(QCompleter::PopupCompletion);
  saved->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
  saved->setMinimumContentsLength(18);
  lib->addWidget(saved, 1);
  lib->addWidget(toolButton("Open", "document-open", library, [this, saved] {
    send({{"op", "palette_open"}, {"id", saved->currentData().toString()}});
  }));
  lib->addWidget(toolButton("Delete", "edit-delete", library, [this, saved] {
    if (QMessageBox::question(this, "Delete palette",
                              "Delete the selected saved palette?") ==
        QMessageBox::Yes)
      send({{"op", "palette_delete"}, {"id", saved->currentData().toString()}});
  }));
  l->addWidget(library);
  auto gen = new QGroupBox("Generate palette", w);
  gen->setObjectName("paletteGenerator");
  gen->installEventFilter(this);
  auto columns = new QBoxLayout(QBoxLayout::LeftToRight, gen);
  auto parameters = new QWidget(gen);
  parameters->setObjectName("paletteParameters");
  auto f = new QFormLayout(parameters);
  f->setContentsMargins(0, 0, 0, 0);
  f->setRowWrapPolicy(QFormLayout::WrapLongRows);
  auto selector = new QWidget(gen);
  selector->setObjectName("paletteSelector");
  auto selectorLayout = new QVBoxLayout(selector);
  selectorLayout->setContentsMargins(0, 0, 0, 0);
  selectorLayout->setSpacing(3);
  columns->addWidget(parameters, 0, Qt::AlignTop);
  columns->addWidget(selector, 0, Qt::AlignTop);
  auto generations = combo("Palette generations", array(p, "generations"), gen);
  generations->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
  generations->setMinimumContentsLength(20);
  // Compact layouts can shrink the selector while wider layouts fit its text.
  generations->setMinimumWidth(generations->minimumSizeHint().width());
  generations->setSizeAdjustPolicy(QComboBox::AdjustToContents);
  int generationIndex =
      generations->findData(generationDraft.value("generation").toString());
  if (generationIndex >= 0)
    generations->setCurrentIndex(generationIndex);
  connect(
      generations, &QComboBox::currentIndexChanged, this, [this, generations] {
        generationDraft["generation"] = generations->currentData().toString();
      });
  auto load = toolButton("Load generation", "document-open-recent", gen, [this, generations] {
    send({{"op", "palette_generation"},
          {"id", generations->currentData().toString()}});
  });
  bool hasGenerations = generations->count() > 0;
  if (!hasGenerations) {
    generations->addItem("No saved generations");
    generations->setEnabled(false);
  }
  load->setEnabled(hasGenerations);
  auto generationRow = new QWidget(gen);
  generationRow->setObjectName("savedGenerationRow");
  auto generationLayout = new QHBoxLayout(generationRow);
  generationLayout->setContentsMargins(0, 0, 0, 0);
  generationLayout->addWidget(generations, 1);
  generationLayout->addWidget(load);
  f->addRow("Saved generations", generationRow);
  f->labelForField(generationRow)->setObjectName("savedGenerationLabel");
  auto mode = new QComboBox(gen);
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
  connect(mode, &QComboBox::currentIndexChanged, this, [this, mode] {
    generationDraft["mode"] = mode->currentData().toString();
  });
  f->addRow("Mode", mode);
  auto count =
      spin("Count", generationDraft.value("count").toInt(5), 1, 64, gen);
  connect(count, &QSpinBox::valueChanged, this,
          [this](int value) { generationDraft["count"] = value; });
  f->addRow("Count", count);
  auto seed =
      line("Random seed", generationDraft.value("seed").toString("0"), gen);
  seed->setMaximumWidth(160);
  connect(seed, &QLineEdit::textChanged, this,
          [this](const QString &value) { generationDraft["seed"] = value; });
  f->addRow("Seed", seed);
  auto base = line("Base color (empty for current)",
                   generationDraft.value("base").toString(), gen);
  base->setMaximumWidth(160);
  connect(base, &QLineEdit::textChanged, this,
          [this](const QString &value) { generationDraft["base"] = value; });
  f->addRow("Base color", base);
  QColor initialBase = color(base->text().isEmpty()
                                 ? str(object(state.value("mixer")), "current")
                                 : base->text());
  auto wheel = new GeneratorWheel(initialBase, [base](QColor c) {
    base->setText(hex(c));
  }, gen);
  connect(base, &QLineEdit::textChanged, wheel, [wheel](const QString &text) {
    wheel->setColor(color(text));
  });
  wheel->setFixedSize(140, 140);
  selectorLayout->addWidget(wheel, 0, Qt::AlignLeft);
  auto hmin = decimal("Minimum hue", generationDraft.value("hmin").toDouble(0),
                      0, 360, gen),
       hmax = decimal("Maximum hue",
                      generationDraft.value("hmax").toDouble(360), 0, 360, gen),
       lmin = decimal("Minimum lightness",
                      generationDraft.value("lmin").toDouble(0), 0, 1, gen),
       lmax = decimal("Maximum lightness",
                      generationDraft.value("lmax").toDouble(1), 0, 1, gen),
       cmin = decimal("Minimum chroma",
                      generationDraft.value("cmin").toDouble(0), 0, .4, gen),
       cmax = decimal("Maximum chroma",
                      generationDraft.value("cmax").toDouble(.4), 0, .4, gen);
  for (auto pair : QList<QPair<QDoubleSpinBox *, QString>>{{hmin, "hmin"},
                                                           {hmax, "hmax"},
                                                           {lmin, "lmin"},
                                                           {lmax, "lmax"},
                                                           {cmin, "cmin"},
                                                           {cmax, "cmax"}})
    {
    pair.first->setMaximumWidth(110);
    connect(
        pair.first, &QDoubleSpinBox::valueChanged, this,
        [this, pair](double value) { generationDraft[pair.second] = value; });
    }
  f->addRow("Hue minimum", hmin);
  f->addRow("Hue maximum", hmax);
  f->addRow("Lightness minimum", lmin);
  f->addRow("Lightness maximum", lmax);
  f->addRow("Chroma minimum", cmin);
  f->addRow("Chroma maximum", cmax);
  auto contrast = line("Contrast (optional)",
                       generationDraft.value("contrast").toString(), gen);
  contrast->setMaximumWidth(110);
  connect(
      contrast, &QLineEdit::textChanged, this,
      [this](const QString &value) { generationDraft["contrast"] = value; });
  f->addRow("Contrast", contrast);
  auto preview = new QWidget(gen);
  auto previewLayout = new QHBoxLayout(preview);
  previewLayout->setContentsMargins(0, 0, 0, 0);
  previewLayout->setSpacing(0);
  auto previewScroll = new QScrollArea(gen);
  previewScroll->setWidgetResizable(true);
  previewScroll->setFixedHeight(38);
  previewScroll->setWidget(preview);
  previewScroll->setAccessibleName("Generated palette preview");
  selectorLayout->addWidget(previewScroll);
  auto previewError = new QLabel(gen);
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
  auto previewTimer = new QTimer(gen);
  previewTimer->setSingleShot(true);
  previewTimer->setInterval(80);
  connect(previewTimer, &QTimer::timeout, gen, [=, this] {
    auto req = makeRequest("palette_preview_generation");
    QJsonObject result = req.isEmpty() ? QJsonObject{{"error", "Enter a valid seed and contrast."}}
                                       : request(req).toObject();
    previewError->setText(str(result, "error"));
    previewError->setVisible(!previewError->text().isEmpty());
    while (auto item = previewLayout->takeAt(0)) {
      if (item->widget()) item->widget()->deleteLater();
      delete item;
    }
    QList<QColor> colors;
    for (const auto &value : array(result, "colors")) {
      auto c = color(value.toString());
      if (!c.isValid()) continue;
      colors.append(c);
      auto swatch = new QLabel(value.toString(), preview);
      swatch->setAccessibleName("Generated color " + value.toString());
      swatch->setAlignment(Qt::AlignCenter);
      swatch->setMinimumWidth(100);
      swatch->setStyleSheet(QString("background-color: %1; color: %2;")
                                .arg(c.name(), c.lightness() < 140 ? "white" : "black"));
      previewLayout->addWidget(swatch);
    }
    wheel->setColors(colors);
    const auto selectedMode = mode->currentData().toString();
    wheel->setHarmonyHandles(selectedMode != "random" && selectedMode != "perceptual-contrast");
  });
  auto schedulePreview = [previewTimer] { previewTimer->start(); };
  connect(mode, &QComboBox::currentIndexChanged, gen, schedulePreview);
  connect(count, &QSpinBox::valueChanged, gen, schedulePreview);
  for (auto edit : {base, seed, contrast})
    connect(edit, &QLineEdit::textChanged, gen, schedulePreview);
  for (auto control : {hmin, hmax, lmin, lmax, cmin, cmax})
    connect(control, &QDoubleSpinBox::valueChanged, gen, schedulePreview);
  previewTimer->start(0);
  selectorLayout->addWidget(button("Generate", gen, [=, this] {
    auto req = makeRequest("palette_generate");
    if (req.isEmpty()) {
      QMessageBox::warning(this, "Invalid generation settings",
                           "Enter a valid non-negative integer seed and numeric contrast.");
      return;
    }
    send(req);
  }));
  l->addWidget(gen);
  auto status = new QLabel(
      str(p, "error").isEmpty() ? str(p, "status") : str(p, "error"), w);
  status->setWordWrap(true);
  l->addWidget(status);
  l->addStretch();
  return w;
}

void Frontend::layoutThemePage(QWidget *page, int width, bool initialize) {
  auto panes = page->findChild<QSplitter *>("themePanes");
  if (!panes)
    return;
  auto preview = panes->widget(0);
  auto inspector = panes->widget(1);
  auto canvas = static_cast<Scene *>(preview->findChild<QWidget *>("themeScene"));
  auto toggle = page->findChild<QAction *>("themePreviewAction");
  const bool compact = width < 680;
  const auto orientation = compact ? Qt::Vertical : Qt::Horizontal;
  toggle->setVisible(compact && canvas);
  preview->setVisible(canvas && (!compact || compactThemePreview));
  if (!initialize && panes->orientation() == orientation)
    return;

  panes->setOrientation(orientation);
  panes->setHandleWidth(compact ? 0 : style()->pixelMetric(QStyle::PM_SplitterWidth));
  panes->handle(1)->setEnabled(!compact);
  inspector->setMinimumWidth(compact ? 0 : 200);
  auto previewLayout = static_cast<QBoxLayout *>(preview->layout());
  previewLayout->setDirection(compact ? QBoxLayout::LeftToRight : QBoxLayout::TopToBottom);
  preview->setMaximumHeight(compact ? page->fontMetrics().height() * 6 : QWIDGETSIZE_MAX);
  if (canvas) {
    canvas->setMaximumWidth(compact ? page->fontMetrics().height() * 16 : QWIDGETSIZE_MAX);
    canvas->setTopAligned(!compact);
  }
  if (compact) {
    panes->setSizes({0, page->height()});
  } else {
    const auto saved = settings.value("board/themeSplitter").toList();
    panes->setSizes(saved.size() == 2 ? QList<int>{saved[0].toInt(), saved[1].toInt()}
                                      : QList<int>{440, 340});
  }
}

QWidget *Frontend::themePage(Scene *reuse) {
  auto t = object(state.value("theme")), project = object(t.value("project"));
  auto outer = new QWidget;
  outer->setObjectName("themePage");
  outer->installEventFilter(this);
  auto outerLayout = new QVBoxLayout(outer);
  outerLayout->setSpacing(4);
  outerLayout->setContentsMargins(5, 5, 5, 5);
  auto projectDialog = new QDialog(outer);
  projectDialog->setObjectName("themeProjectDialog");
  projectDialog->setWindowTitle("Theme project");
  auto projectLayout = new QVBoxLayout(projectDialog);
  auto w = new QWidget;
  auto l = new QVBoxLayout(w);
  l->setContentsMargins(0, 0, 0, 0);
  auto selection = new QFormLayout;
  selection->setRowWrapPolicy(QFormLayout::WrapLongRows);
  auto plugins =
      combo("Theme plugins", array(t, "plugins"), w,
            themeDraft.value("plugin").toString(str(project, "plugin_id")));
  connect(plugins, &QComboBox::currentIndexChanged, this, [this, plugins] {
    themeDraft["plugin"] = plugins->currentData().toString();
  });
  auto pluginRow = new QWidget(w);
  auto pluginLayout = new QHBoxLayout(pluginRow);
  pluginLayout->setContentsMargins(0, 0, 0, 0);
  pluginLayout->addWidget(plugins, 1);
  auto discard = [this, project] {
    return !project.value("dirty").toBool() ||
           QMessageBox::question(this, "Unsaved theme",
                                 "Discard unsaved theme changes?") ==
               QMessageBox::Yes;
  };
  pluginLayout->addWidget(toolButton("New theme", "document-new", w, [this, plugins, discard] {
    if (discard())
      send({{"op", "theme_choose"},
            {"id", plugins->currentData().toString()},
            {"discard", true}});
  }));
  selection->addRow("Plugin", pluginRow);
  auto saved = combo("Saved themes", array(t, "saved"), w,
                     themeDraft.value("saved").toString());
  connect(saved, &QComboBox::currentIndexChanged, this, [this, saved] {
    themeDraft["saved"] = saved->currentData().toString();
  });
  auto savedRow = new QWidget(w);
  auto savedLayout = new QHBoxLayout(savedRow);
  savedLayout->setContentsMargins(0, 0, 0, 0);
  savedLayout->addWidget(saved, 1);
  savedLayout->addWidget(toolButton("Open theme", "document-open", w, [this, saved, discard] {
    if (discard())
      send({{"op", "theme_open"},
            {"id", saved->currentData().toString()},
            {"discard", true}});
  }));
  savedLayout->addWidget(
      toolButton("Reload", "view-refresh", w, [this] { send({{"op", "theme_reload"}}); }));
  selection->addRow("Saved", savedRow);
  auto name = line("Theme name", str(project, "name"), w);
  name->setMaximumWidth(name->fontMetrics().horizontalAdvance(QString(32, QLatin1Char('M'))) + 20);
  connect(name, &QLineEdit::editingFinished, this, [this, name] {
    send({{"op", "theme_name"}, {"name", name->text()}});
  });
  selection->addRow("Name", name);
  projectLayout->addLayout(selection);
  auto closeProject = new QDialogButtonBox(QDialogButtonBox::Close, projectDialog);
  connect(closeProject, &QDialogButtonBox::rejected, projectDialog, &QDialog::reject);
  connect(projectDialog, &QDialog::finished, this, [this] { themeProjectVisible = false; });
  projectLayout->addWidget(closeProject);
  auto actions = new QToolBar(outer);
  actions->setMovable(false);
  for (auto pair : QList<QPair<QString, QString>>{
           {"Save", "theme_save"},
           {"Undo", "theme_undo"},
           {"Redo", "theme_redo"},
           {"Use active palette", "theme_palette_bind"}}) {
    auto b =
        toolButton(pair.first, actionIcon(pair.second), actions, [this, pair] { send({{"op", pair.second}}); });
    if (pair.second == "theme_undo")
      b->setEnabled(t.value("can_undo").toBool());
    if (pair.second == "theme_redo")
      b->setEnabled(t.value("can_redo").toBool());
    actions->addWidget(b);
  }
  outerLayout->addWidget(actions);
  auto panes = new QSplitter(Qt::Vertical, outer);
  panes->setObjectName("themePanes");
  panes->setChildrenCollapsible(false);
  auto previewContainer = new QWidget(panes);
  previewContainer->setObjectName("themePreviewContainer");
  auto previewLayout = new QBoxLayout(QBoxLayout::TopToBottom, previewContainer);
  previewLayout->setContentsMargins(0, 0, 0, 0);
  auto scene =
      combo("Preview scene", array(t, "scenes"), previewContainer, str(project, "scene"));
  scene->setSizeAdjustPolicy(QComboBox::AdjustToContents);
  scene->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Fixed);
  connect(scene, &QComboBox::activated, this, [this, scene](int) {
    send({{"op", "theme_scene"}, {"id", scene->currentData().toString()}});
  });
  auto sceneSelector = new QWidget(previewContainer);
  auto sceneControls = new QHBoxLayout(sceneSelector);
  sceneControls->setContentsMargins(0, 0, 0, 0);
  auto sceneLabel = new QLabel("Preview scene", sceneSelector);
  sceneLabel->setBuddy(scene);
  sceneControls->addWidget(sceneLabel);
  sceneControls->addWidget(scene);
  sceneControls->addStretch();
  previewLayout->addWidget(sceneSelector, 0, Qt::AlignTop);
  if (t.value("scene").isObject()) {
    Scene *preview;
    if (reuse) {
      reuse->setParent(previewContainer);
      preview = reuse;
    } else {
      preview = new Scene(
          t.value("scene").toObject(), [this](QJsonObject o) { send(o); }, previewContainer);
    }
    preview->setObjectName("themeScene");
    previewLayout->addWidget(preview, 1);
  }
  auto previewToggle = new QToolButton(actions);
  previewToggle->setObjectName("themePreviewToggle");
  previewToggle->setText("Show preview");
  previewToggle->setAccessibleName("Show preview");
  previewToggle->setCheckable(true);
  previewToggle->setChecked(compactThemePreview);
  connect(previewToggle, &QToolButton::toggled, this, [this, previewContainer](bool visible) {
    compactThemePreview = visible;
    previewContainer->setVisible(visible);
  });
  actions->insertWidget(actions->actions().first(), previewToggle)->setObjectName("themePreviewAction");
  auto projectButton = toolButton("Project", "document-properties", actions, [this, projectDialog] {
    themeProjectVisible = true;
    projectDialog->show();
    projectDialog->raise();
    projectDialog->activateWindow();
  });
  actions->insertWidget(actions->actions().first(), projectButton);
  if (themeProjectVisible) projectDialog->show();
  auto roles = new QGroupBox("Theme roles", w);
  auto rl = new QVBoxLayout(roles);
  for (auto value : array(t, "roles")) {
    auto o = value.toObject();
    auto id = str(o, "id");
    auto row = new QWidget(roles);
    const bool selected = str(t, "selected") == id;
    if (selected)
      row->setObjectName("selectedThemeRole");
    auto r = new QGridLayout(row);
    r->setContentsMargins(0, 0, 0, 0);
    auto sw = new Swatch(color(str(o, "color")), str(o, "name"), row);
    sw->setCheckable(true);
    sw->setChecked(selected);
    r->addWidget(sw, 0, 0, 1, 2, Qt::AlignLeft);
    connect(sw, &QPushButton::clicked, this,
            [this, id, sw, selected] {
              sw->setChecked(selected);
              send({{"op", "theme_select"}, {"role", id}});
            });
    auto edit = new ColorLineEdit(str(o, "value"), row);
    edit->setMaximumWidth(edit->fontMetrics().horizontalAdvance(QString(12, QLatin1Char('M'))) + 20);
    r->addWidget(edit, 0, 2);
    connect(edit, &QLineEdit::editingFinished, this, [this, id, edit] {
      send({{"op", "theme_assign"}, {"role", id}, {"color", edit->text()}});
    });
    r->addWidget(button("Default", row, [this, id] {
      send({{"op", "theme_default"}, {"role", id}});
    }), 1, 2, Qt::AlignRight);
    auto link = new QComboBox(row);
    link->setAccessibleName("Palette swatch link");
    link->setEditable(true);
    for (auto v : array(t, "palette_swatches"))
      link->addItem(str(v.toObject(), "name"));
    link->setEditText(str(o, "link"));
    link->setMinimumContentsLength(9);
    link->setSizeAdjustPolicy(QComboBox::AdjustToMinimumContentsLengthWithIcon);
    r->addWidget(new QLabel("Palette link", row), 1, 0);
    r->addWidget(link, 1, 1);
    r->setColumnStretch(1, 1);
    connect(link, &QComboBox::activated, this, [this, id, link](int) {
      send({{"op", "theme_link"}, {"role", id}, {"name", link->currentText()}});
    });
    connect(link->lineEdit(), &QLineEdit::returnPressed, this,
            [this, id, link] {
              send({{"op", "theme_link"},
                    {"role", id},
                    {"name", link->currentText()}});
            });
    rl->addWidget(row);
  }
  l->addWidget(roles);
  for (auto value : array(t, "preview")) {
    auto o = value.toObject();
    auto group = new QGroupBox(str(o, "path"), w);
    auto gl = new QVBoxLayout(group);
    auto text = new QPlainTextEdit(str(o, "content"), group);
    text->setReadOnly(true);
    text->setAccessibleName("Rendered " + str(o, "path"));
    gl->addWidget(text);
    l->addWidget(group);
  }
  auto exportRow = new QGridLayout;
  auto dir =
      line("Export directory", themeDraft.value("directory").toString(), w);
  connect(dir, &QLineEdit::textChanged, this,
          [this](const QString &value) { themeDraft["directory"] = value; });
  exportRow->addWidget(dir, 0, 0, 1, 2);
  exportRow->addWidget(button("Choose directory", w, [this, dir] {
    auto path =
        QFileDialog::getExistingDirectory(this, "Export theme", dir->text());
    if (!path.isEmpty())
      dir->setText(path);
  }), 0, 2);
  auto overwrite = new QCheckBox("Overwrite existing files", w);
  overwrite->setChecked(themeDraft.value("overwrite").toBool());
  connect(overwrite, &QCheckBox::toggled, this,
          [this](bool value) { themeDraft["overwrite"] = value; });
  exportRow->addWidget(overwrite, 1, 0);
  exportRow->addWidget(button("Export", w, [this, dir, overwrite] {
    if (dir->text().trimmed().isEmpty()) {
      QMessageBox::warning(this, "Export theme", "Choose an export directory.");
      return;
    }
    send({{"op", "theme_export"},
          {"directory", dir->text()},
          {"overwrite", overwrite->isChecked()}});
  }), 1, 2);
  l->addLayout(exportRow);
  auto error = new QLabel(str(t, "error"), w);
  error->setWordWrap(true);
  l->addWidget(error);
  l->addStretch();
  auto controlsScroll = scroll(w);
  controlsScroll->setObjectName("themeInspector");
  controlsScroll->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Ignored);
  controlsScroll->setMinimumHeight(48);
  panes->addWidget(previewContainer);
  panes->addWidget(controlsScroll);
  connect(panes, &QSplitter::splitterMoved, this, [this, panes] {
    if (panes->orientation() == Qt::Horizontal) {
      const auto sizes = panes->sizes();
      settings.setValue("board/themeSplitter", QVariantList{sizes[0], sizes[1]});
    }
  });
  outerLayout->addWidget(panes, 1);
  layoutThemePage(outer, tabs ? tabs->width() : outer->width(), true);
  return outer;
}

void Frontend::renderConfig() {
  if (configSave) {
    configSave->setEnabled(state.value("load_error").isNull() &&
                           !state.value("busy").toBool());
    configRebind->setEnabled(!state.value("busy").toBool());
    configStatus->setText(str(state, "validation_field").isEmpty() ? message() : QString());
    configStatus->setVisible(!configStatus->text().isEmpty());
    configStatus->setToolTip(str(state, "message_detail"));
    configStatus->setAccessibleDescription(str(state, "message_detail"));
    shortcutStatus->setText(str(state, "shortcut_status"));
    shortcutStatus->setToolTip(str(state, "shortcut_status_detail"));
    shortcutStatus->setAccessibleDescription(str(state, "shortcut_status_detail"));
    limitError->setText(str(state, "message"));
    limitError->setVisible(str(state, "validation_field") == "history_limit");
    shortcutStatus->setVisible(!shortcutStatus->text().isEmpty());
    return;
  }
  if (!configInitialized && state.value("config").isObject()) {
    configDraft = state.value("config").toObject();
    configInitialized = true;
  }
  auto w = new QWidget;
  auto l = new QVBoxLayout(w);
  auto f = new QFormLayout;
  f->setRowWrapPolicy(QFormLayout::WrapLongRows);
  f->setFieldGrowthPolicy(QFormLayout::FieldsStayAtSizeHint);
  auto limit =
      line("Stored history limit", str(configDraft, "history_limit"), w);
  limit->setMaximumWidth(limit->fontMetrics().horizontalAdvance(QString(20, QLatin1Char('0'))) + 24);
  limit->setValidator(new QRegularExpressionValidator(
      QRegularExpression("[0-9]{1,20}"), limit));
  configLimit = limit;
  f->addRow("Stored history limit", limit);
  limitError = new QLabel("Stored history limit must be greater than zero.", w);
  limitError->setWordWrap(true);
  limitError->setStyleSheet("color: palette(link);");
  limitError->setVisible(false);
  f->addRow(QString(), limitError);
  connect(limit, &QLineEdit::textChanged, this,
          [this](const QString &v) { configDraft["history_limit"] = v; });
  auto number = [&](const char *key, const QString &label, int min, int max) {
    auto s = spin(label, configDraft.value(key).toInt(), min, max, w);
    s->setMaximumWidth(s->sizeHint().width());
    f->addRow(label, s);
    connect(s, &QSpinBox::valueChanged, this,
            [this, key](int v) { configDraft[key] = v; });
  };
  number("history_display_limit", "Displayed history limit", 1, 10000);
  number("history_columns", "History columns", 1, 12);
  number("history_rows", "History rows", 1, 12);
  number("picker_zoom", "Picker zoom", 1, 32);
  auto check = [&](const char *key, const QString &label) {
    auto c = new QCheckBox(label, w);
    c->setChecked(configDraft.value(key).toBool());
    f->addRow(c);
    connect(c, &QCheckBox::toggled, this,
            [this, key](bool v) { configDraft[key] = v; });
  };
  check("auto_copy", "Automatically copy picked colors");
  check("hide_board_while_picking", "Hide board while picking");
  check("autostart", "Start automatically");
  auto format = new QComboBox(w);
  format->setAccessibleName("Clipboard format");
  format->setSizeAdjustPolicy(QComboBox::AdjustToContents);
  for (auto value : {"hex", "hex-rgba", "rgb", "rgba", "hsl", "hsv", "cmyk"})
    format->addItem(QString(value).toUpper(), value);
  format->setCurrentIndex(
      qMax(0, format->findData(str(configDraft, "clipboard_format"))));
  connect(format, &QComboBox::currentIndexChanged, this, [this, format] {
    configDraft["clipboard_format"] = format->currentData().toString();
  });
  f->addRow("Clipboard format", format);
  auto text = [&](const char *key, const QString &label) {
    auto e = line(label, str(configDraft, key), w);
    e->setMaximumWidth(e->fontMetrics().horizontalAdvance("Ctrl+Shift+Meta+F12") + 24);
    f->addRow(label, e);
    connect(e, &QLineEdit::textChanged, this,
            [this, key](const QString &v) { configDraft[key] = v; });
  };
  text("pick_shortcut", "Pick shortcut");
  text("history_shortcut", "History shortcut");
  l->addLayout(f);
  auto row = new QHBoxLayout;
  configSave = button("Save settings", w, [this] {
    send({{"op", "save"}, {"config", configDraft}});
    if (str(state, "validation_field") == "history_limit") {
      configLimit->setFocus();
      configScroll->ensureWidgetVisible(configLimit);
    }
  });
  configSave->setEnabled(state.value("load_error").isNull() &&
                         !state.value("busy").toBool());
  configSave->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Preferred);
  row->addWidget(configSave);
  configRebind =
      button("Rebind shortcuts", w, [this] { send({{"op", "rebind"}}); });
  configRebind->setEnabled(!state.value("busy").toBool());
  configRebind->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Preferred);
  row->addWidget(configRebind);
  auto closeButton = button("Close", w, [this] { close(); });
  closeButton->setSizePolicy(QSizePolicy::Maximum, QSizePolicy::Preferred);
  row->addWidget(closeButton);
  row->addStretch();
  l->addLayout(row);
  configStatus = new QLabel(message(), w);
  configStatus->setWordWrap(true);
  configStatus->setTextFormat(Qt::PlainText);
  configStatus->setVisible(!configStatus->text().isEmpty());
  configStatus->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
  l->addWidget(configStatus);
  shortcutStatus = new QLabel(str(state, "shortcut_status"), w);
  shortcutStatus->setToolTip(str(state, "shortcut_status_detail"));
  shortcutStatus->setAccessibleDescription(str(state, "shortcut_status_detail"));
  shortcutStatus->setWordWrap(true);
  shortcutStatus->setTextFormat(Qt::PlainText);
  shortcutStatus->setVisible(!shortcutStatus->text().isEmpty());
  shortcutStatus->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
  l->addWidget(shortcutStatus);
  l->addStretch();
  configScroll = static_cast<QScrollArea *>(scroll(w));
  replace(configScroll);
}
void Frontend::renderHistory() {
  historyButtons.clear();
  auto w = new QWidget;
  auto l = new QVBoxLayout(w);
  auto grid = new QGridLayout;
  int columns = qMax(1, state.value("columns").toInt(4));
  int index = 0;
  for (auto value : array(state, "entries")) {
    auto o = value.toObject();
    auto b = new Swatch(color(str(o, "color")), str(o, "text"), w);
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
  auto error = new QLabel(str(state, "error"), w);
  error->setWordWrap(true);
  l->addWidget(error);
  auto sc = scroll(w);
  int rows = qMax(1, state.value("rows").toInt(3));
  sc->setMinimumSize(qMin(900, columns * 110), qMin(650, rows * 55 + 35));
  replace(sc);
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
