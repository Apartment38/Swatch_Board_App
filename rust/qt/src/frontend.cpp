#include <QtCore>
#include <QtGui>
#include <QtWidgets>
#include <functional>

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
class PaletteDrop : public QLabel {
  std::function<void(int, QString)> append;

public:
  PaletteDrop(std::function<void(int, QString)> f, QWidget *parent)
      : QLabel("Drop a history color here to add it", parent), append(f) {
    setAcceptDrops(true);
    setAccessibleName("Drop a history color to add it to the palette");
    setMinimumHeight(32);
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
  int h, s, v;
  std::function<void(int, int, int, bool)> changed;
  void choose(QPoint p, bool final) {
    s = qBound(0, p.x() * 100 / qMax(1, width() - 1), 100);
    v = qBound(0, 100 - p.y() * 100 / qMax(1, height() - 1), 100);
    update();
    changed(h, s, v, final);
  }

public:
  Spectrum(int hue, int sat, int val,
           std::function<void(int, int, int, bool)> f, QWidget *parent)
      : QWidget(parent), h(hue), s(sat), v(val), changed(f) {
    setMinimumSize(180, 120);
    setFocusPolicy(Qt::StrongFocus);
    setAccessibleName("Saturation and value spectrum; arrow keys adjust");
  }

protected:
  void paintEvent(QPaintEvent *) override {
    QPainter p(this);
    QLinearGradient a(0, 0, width(), 0);
    a.setColorAt(0, Qt::white);
    a.setColorAt(1, QColor::fromHsv(h % 360, 255, 255));
    p.fillRect(rect(), a);
    QLinearGradient b(0, 0, 0, height());
    b.setColorAt(0, Qt::transparent);
    b.setColorAt(1, Qt::black);
    p.fillRect(rect(), b);
    p.setPen(Qt::white);
    p.drawEllipse(
        QPoint(s * (width() - 1) / 100, (100 - v) * (height() - 1) / 100), 5,
        5);
  }
  void mousePressEvent(QMouseEvent *e) override { choose(e->pos(), false); }
  void mouseMoveEvent(QMouseEvent *e) override {
    if (e->buttons() & Qt::LeftButton)
      choose(e->pos(), false);
  }
  void mouseReleaseEvent(QMouseEvent *e) override { choose(e->pos(), true); }
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
    s = qBound(0, s + ds, 100);
    v = qBound(0, v + dv, 100);
    update();
    changed(h, s, v, true);
  }
};
class Scene : public QWidget {
  QJsonObject scene;
  QVector<QImage> images;
  std::function<void(QJsonObject)> send;
  QRectF targetRect() const {
    double w = scene.value("width").toDouble(1),
           h = scene.value("height").toDouble(1);
    if (w <= 0 || h <= 0)
      return {};
    double scale = qMin(width() / w, height() / h);
    return QRectF((width() - w * scale) / 2, (height() - h * scale) / 2,
                  w * scale, h * scale);
  }
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
    setMinimumSize(300, 220);
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
  QPushButton *pickButton = nullptr;
  QVBoxLayout *root;
  QVBoxLayout *historyLayout = nullptr;
  QWidget *historyBox = nullptr;
  QScrollArea *historyScroll = nullptr;
  QSplitter *splitter = nullptr, *verticalSplitter = nullptr;
  QWidget *pinnedScroll = nullptr;
  QPushButton *configSave = nullptr, *configRebind = nullptr;
  QLabel *configStatus = nullptr, *shortcutStatus = nullptr;
  QList<QPushButton *> historyButtons;
  bool hiddenForPick = false, spectrumDragging = false;
  QJsonObject configDraft, generationDraft, themeDraft;
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
  void preserveView(QWidget *old, QWidget *replacement) {
    auto oldScroll = qobject_cast<QScrollArea *>(old),
         newScroll = qobject_cast<QScrollArea *>(replacement);
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
  QWidget *scroll(QWidget *content) {
    auto s = new QScrollArea;
    s->setWidgetResizable(true);
    s->setWidget(content);
    return s;
  }
  QWidget *pinnedPalette() {
    auto p = object(state.value("palette"));
    auto w = new QGroupBox("Active palette");
    auto l = new QVBoxLayout(w);
    auto controls = new QHBoxLayout;
    controls->addWidget(new QLabel(str(p, "name"), w));
    for (auto pair :
         QList<QPair<QString, QString>>{{"Add current", "palette_add_current"},
                                        {"Undo", "palette_undo"},
                                        {"Redo", "palette_redo"}}) {
      auto b =
          button(pair.first, w, [this, pair] { send({{"op", pair.second}}); });
      if (pair.second == "palette_undo")
        b->setEnabled(p.value("can_undo").toBool());
      if (pair.second == "palette_redo")
        b->setEnabled(p.value("can_redo").toBool());
      controls->addWidget(b);
    }
    controls->addStretch();
    l->addLayout(controls);
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
          },
          w));
    auto swatchRow = new QHBoxLayout;
    l->addLayout(swatchRow);
    int i = 0;
    for (auto value : swatches) {
      auto o = value.toObject();
      int index = i++;
      auto box = new QGroupBox(QString::number(index + 1), w);
      auto layout = new QVBoxLayout(box);
      auto sw = new Swatch(color(str(o, "color")), str(o, "name"), box);
      sw->paletteDrop(index, [this, index, o](int from, QString c) {
        if (from >= 0)
          send({{"op", "palette_move"}, {"from", from}, {"to", index}});
        else if (color(c).isValid())
          send({{"op", "palette_edit"},
                {"index", index},
                {"name", str(o, "name")},
                {"color", c}});
      });
      layout->addWidget(sw);
      auto name = line("Pinned swatch name", str(o, "name"), box),
           c = line("Pinned swatch color", str(o, "color"), box);
      auto fields = new QHBoxLayout;
      fields->addWidget(name);
      fields->addWidget(c);
      layout->addLayout(fields);
      auto apply = [this, index, name, c] {
        send({{"op", "palette_edit"},
              {"index", index},
              {"name", name->text()},
              {"color", c->text()}});
      };
      connect(name, &QLineEdit::editingFinished, this, apply);
      connect(c, &QLineEdit::editingFinished, this, apply);
      auto lock = new QCheckBox("Locked", box);
      lock->setChecked(o.value("locked").toBool());
      connect(lock, &QCheckBox::toggled, this, [this, index](bool v) {
        send({{"op", "palette_lock"}, {"index", index}, {"locked", v}});
      });
      auto row = new QHBoxLayout;
      row->addWidget(lock);
      row->addWidget(button("Up", box, [this, index] {
        if (index > 0)
          send({{"op", "palette_move"}, {"from", index}, {"to", index - 1}});
      }));
      row->addWidget(button("Down", box, [this, index, swatches] {
        if (index + 1 < swatches.size())
          send({{"op", "palette_move"}, {"from", index}, {"to", index + 1}});
      }));
      row->addWidget(button("Delete", box, [this, index] {
        send({{"op", "palette_delete_swatch"}, {"index", index}});
      }));
      layout->addLayout(row);
      swatchRow->addWidget(box);
    }
    swatchRow->addStretch();
    return w;
  }
  void renderBoard(const QJsonObject &previous) {
    bool first = !tabs;
    if (first) {
      auto outer = new QWidget;
      auto layout = new QVBoxLayout(outer);
      auto top = new QHBoxLayout;
      pickButton = button("Pick color", outer, [this] {
        if (state.value("hide_board_while_picking").toBool()) {
          hiddenForPick = true;
          hide();
          QGuiApplication::sync();
        }
        send({{"op", "pick"}});
      });
      top->addWidget(pickButton);
      top->addWidget(
          button("Settings", outer, [this] { send({{"op", "configure"}}); }));
      connectionStatus = new QLabel(outer);
      top->addWidget(connectionStatus);
      auto toggle = new QCheckBox("Show history", outer);
      toggle->setChecked(settings.value("board/historyVisible", true).toBool());
      top->addWidget(toggle);
      top->addStretch();
      layout->addLayout(top);
      splitter = new QSplitter(Qt::Horizontal, outer);
      historyBox = new QGroupBox("History");
      historyLayout = new QVBoxLayout(historyBox);
      historyScroll = new QScrollArea;
      historyScroll->setWidgetResizable(true);
      historyScroll->setWidget(historyBox);
      historyScroll->setVisible(toggle->isChecked());
      connect(toggle, &QCheckBox::toggled, this, [this](bool visible) {
        historyScroll->setVisible(visible);
        settings.setValue("board/historyVisible", visible);
      });
      splitter->addWidget(historyScroll);
      tabs = new QTabWidget(outer);
      tabs->setAccessibleName("Board sections");
      tabs->addTab(scroll(mixer()), "Mixer");
      tabs->addTab(scroll(palettePage()), "Palette");
      tabs->addTab(scroll(themePage()), "Theme");
      tabs->setCurrentIndex(
          qBound(0, settings.value("board/tab", 0).toInt(), 2));
      connect(tabs, &QTabWidget::currentChanged, this,
              [this](int i) { settings.setValue("board/tab", i); });
      splitter->addWidget(tabs);
      verticalSplitter = new QSplitter(Qt::Vertical, outer);
      verticalSplitter->addWidget(splitter);
      pinnedScroll = scroll(pinnedPalette());
      verticalSplitter->addWidget(pinnedScroll);
      splitter->setStretchFactor(1, 1);
      verticalSplitter->setStretchFactor(0, 1);
      splitter->restoreState(
          settings.value("board/horizontalSplitter").toByteArray());
      if (!verticalSplitter->restoreState(
              settings.value("board/verticalSplitter").toByteArray()))
        verticalSplitter->setSizes({450, 250});
      layout->addWidget(verticalSplitter, 1);
      status = new QLabel(outer);
      status->setWordWrap(true);
      layout->addWidget(status);
      replace(outer);
    } else {
      int current = tabs->currentIndex();
      QSignalBlocker blocker(tabs);
      auto refresh = [this](int index, QWidget *page, const QString &title) {
        auto old = tabs->widget(index);
        auto replacement = scroll(page);
        preserveView(old, replacement);
        tabs->removeTab(index);
        tabs->insertTab(index, replacement, title);
        old->deleteLater();
      };
      if (!spectrumDragging && previous.value("mixer") != state.value("mixer"))
        refresh(0, mixer(), "Mixer");
      if (previous.value("palette") != state.value("palette"))
        refresh(1, palettePage(), "Palette");
      if (previous.value("theme") != state.value("theme")) {
        Scene *reused = nullptr;
        if (object(previous.value("theme")).value("scene") ==
            object(state.value("theme")).value("scene")) {
          for (auto widget : tabs->widget(2)->findChildren<QWidget *>())
            if ((reused = dynamic_cast<Scene *>(widget)))
              break;
        }
        refresh(2, themePage(reused), "Theme");
      }
      if (previous.value("palette") != state.value("palette")) {
        auto replacement = scroll(pinnedPalette());
        preserveView(pinnedScroll, replacement);
        auto old = verticalSplitter->replaceWidget(1, replacement);
        pinnedScroll = replacement;
        old->deleteLater();
      }
      tabs->setCurrentIndex(current);
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
    pickButton->setEnabled(state.value("loaded").toBool() &&
                           state.value("connected").toBool() &&
                           !state.value("picking").toBool());
    connectionStatus->setText(state.value("picking").toBool()   ? "Picking…"
                              : !state.value("loaded").toBool() ? "Loading…"
                              : state.value("connected").toBool()
                                  ? "Connected"
                                  : "Disconnected");
  }
  QWidget *mixer() {
    auto m = object(state.value("mixer"));
    auto w = new QWidget;
    auto l = new QVBoxLayout(w);
    auto hsv = array(m, "hsv"), rgba = array(m, "rgba");
    double h = hsv[0].toDouble(), s = hsv[1].toDouble(), v = hsv[2].toDouble();
    auto current = new Swatch(color(str(m, "current")), "Current color", w);
    current->paletteDrop(-1, [this](int, QString c) {
      if (color(c).isValid())
        send({{"op", "mixer_select"}, {"color", c}});
    });
    l->addWidget(current);
    l->addWidget(new Spectrum(
        qRound(h), qRound(s), qRound(v),
        [this](int h, int s, int v, bool final) {
          spectrumDragging = !final;
          send({{"op", "mixer_hsv"}, {"h", h}, {"s", s}, {"v", v}});
          if (final) {
            auto previous = state;
            previous.remove("mixer");
            renderBoard(previous);
          }
        },
        w));
    auto form = new QFormLayout;
    auto hbox = new QWidget(w);
    auto hl = new QHBoxLayout(hbox);
    hl->setContentsMargins(0, 0, 0, 0);
    auto hh = decimal("Hue", h, 0, 360, hbox),
         ss = decimal("Saturation", s, 0, 100, hbox),
         vv = decimal("Value", v, 0, 100, hbox);
    for (auto x : {hh, ss, vv}) {
      x->setDecimals(6);
      x->setValue(x == hh ? h : x == ss ? s : v);
      x->setSingleStep(.1);
      hl->addWidget(x);
      connect(x, &QDoubleSpinBox::editingFinished, this,
              [this, hh, ss, vv, h, s, v] {
                if (qAbs(hh->value() - h) < 0.00000051 &&
                    qAbs(ss->value() - s) < 0.00000051 &&
                    qAbs(vv->value() - v) < 0.00000051)
                  return;
                send({{"op", "mixer_hsv"},
                      {"h", hh->value()},
                      {"s", ss->value()},
                      {"v", vv->value()}});
              });
    }
    form->addRow("Hue / saturation / value", hbox);
    auto rw = new QWidget(w);
    auto rl = new QHBoxLayout(rw);
    rl->setContentsMargins(0, 0, 0, 0);
    QList<QSpinBox *> channels;
    const char *names[] = {"Red", "Green", "Blue", "Alpha"};
    for (int i = 0; i < 4; i++) {
      auto x = spin(names[i], rgba[i].toInt(), 0, 255, rw);
      channels << x;
      rl->addWidget(x);
    }
    for (auto x : channels)
      connect(x, &QSpinBox::editingFinished, this, [this, channels] {
        send({{"op", "mixer_rgba"},
              {"r", channels[0]->value()},
              {"g", channels[1]->value()},
              {"b", channels[2]->value()},
              {"a", channels[3]->value()}});
      });
    form->addRow("Red / green / blue / alpha", rw);
    auto fields = array(m, "fields");
    const char *labels[] = {"HEX", "RGB", "RGBA", "HSL", "HSV"};
    for (int i = 0; i < 5; i++) {
      auto row = new QWidget(w);
      auto layout = new QHBoxLayout(row);
      layout->setContentsMargins(0, 0, 0, 0);
      auto e = line(labels[i], fields[i].toString(), row);
      layout->addWidget(e);
      auto apply = [this, e, i] {
        e->setModified(false);
        send({{"op", "mixer_apply"}, {"index", i}, {"text", e->text()}});
      };
      connect(e, &QLineEdit::returnPressed, this, apply);
      layout->addWidget(button(QString("Apply %1").arg(labels[i]), row, apply));
      form->addRow(labels[i], row);
    }
    form->addRow("CMYK", new QLabel(str(m, "cmyk"), w));
    form->addRow("Output", new QLabel(str(m, "output"), w));
    l->addLayout(form);
    auto actions = new QHBoxLayout;
    for (auto pair : QList<QPair<QString, QString>>{
             {"Copy", "mixer_copy"},
             {"Add to palette", "palette_add_current"},
             {"Undo", "mixer_undo"},
             {"Redo", "mixer_redo"},
             {"Add source", "mixer_source_add"},
             {"Mix", "mixer_mix"}}) {
      auto b =
          button(pair.first, w, [this, pair] { send({{"op", pair.second}}); });
      if (pair.second == "mixer_undo")
        b->setEnabled(m.value("can_undo").toBool());
      if (pair.second == "mixer_redo")
        b->setEnabled(m.value("can_redo").toBool());
      actions->addWidget(b);
    }
    l->addLayout(actions);
    auto space = new QComboBox(w);
    space->setAccessibleName("Mixing color space");
    space->addItem("sRGB", "srgb");
    space->addItem("OKLCH", "oklch");
    space->setCurrentIndex(qMax(0, space->findData(str(m, "space"))));
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
      auto weight =
          decimal("Source weight", o.value("weight").toDouble(), 0, 1, row);
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
  void renderConfig();
  void renderHistory();

protected:
  bool eventFilter(QObject *watched, QEvent *event) override {
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
      auto shot = [&](const QString &name) {
        if (layout())
          layout()->activate();
        QCoreApplication::sendPostedEvents(nullptr, QEvent::LayoutRequest);
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
        if (layout())
          layout()->activate();
        check(grab().save(dir + "/" + name + ".png"), "Screenshot " + name);
      };
      auto click = [&](QWidget *root, const QString &name) {
        for (auto b : root->findChildren<QPushButton *>())
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
        edit(tabs->widget(0), "HEX", "#01020342");
        click(tabs->widget(0), "Apply HEX");
        check(str(object(state.value("mixer")), "current") == "#01020342",
              "Apply HEX");
        edit(tabs->widget(0), "RGB", "rgb(4, 5, 6)");
        click(tabs->widget(0), "Apply RGB");
        check(str(object(state.value("mixer")), "current") == "#04050642",
              "RGB preserves alpha");
        click(tabs->widget(0), "Undo");
        check(str(object(state.value("mixer")), "current") == "#01020342",
              "Undo");
        click(tabs->widget(0), "Redo");
        check(str(object(state.value("mixer")), "current") == "#04050642",
              "Redo");
        shot("mixer");
        tabs->setCurrentIndex(1);
        click(tabs->widget(1), "New draft");
        click(pinnedScroll, "Add current");
        check(!array(object(state.value("palette")), "swatches").isEmpty(),
              "Add swatch");
        shot("palette");
        auto checks = pinnedScroll->findChildren<QCheckBox *>();
        for (auto c : checks)
          if (c->text() == "Locked") {
            c->click();
            break;
          }
        auto edits = pinnedScroll->findChildren<QLineEdit *>();
        for (auto e : edits)
          if (e->accessibleName() == "Pinned swatch color") {
            e->setText("#010203FF");
            QMetaObject::invokeMethod(e, "editingFinished");
            break;
          }
        check(!str(object(state.value("palette")), "error").isEmpty(),
              "Locked edit refusal");
        checks = pinnedScroll->findChildren<QCheckBox *>();
        for (auto c : checks)
          if (c->text() == "Locked") {
            c->click();
            break;
          }
        edits = pinnedScroll->findChildren<QLineEdit *>();
        for (auto e : edits)
          if (e->accessibleName() == "Pinned swatch color") {
            e->setText("#010203FF");
            QMetaObject::invokeMethod(e, "editingFinished");
            break;
          }
        check(
            str(array(object(state.value("palette")), "swatches")[0].toObject(),
                "color") == "#010203FF",
            "Unlocked edit");
        edits = tabs->widget(1)->findChildren<QLineEdit *>();
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
        if (!array(object(state.value("theme")), "plugins").isEmpty()) {
          click(tabs->widget(2), "New theme");
          click(tabs->widget(2), "Render preview");
        }
        shot("theme");
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
  auto name = line("Palette name", str(p, "name"), w);
  l->addWidget(name);
  connect(name, &QLineEdit::editingFinished, this, [this, name] {
    send({{"op", "palette_name"}, {"name", name->text()}});
  });
  auto actions = new QHBoxLayout;
  for (auto pair :
       QList<QPair<QString, QString>>{{"New draft", "palette_new"},
                                      {"Save", "palette_save"},
                                      {"Duplicate", "palette_duplicate"}}) {
    auto b =
        button(pair.first, w, [this, pair] { send({{"op", pair.second}}); });
    actions->addWidget(b);
  }
  l->addLayout(actions);
  auto historyLock = new QCheckBox("Lock history additions", w);
  historyLock->setChecked(p.value("lock_history").toBool());
  connect(historyLock, &QCheckBox::toggled, this, [this](bool b) {
    send({{"op", "palette_lock_history"}, {"locked", b}});
  });
  l->addWidget(historyLock);
  auto library = new QGroupBox("Palette library", w);
  auto lib = new QHBoxLayout(library);
  auto saved =
      combo("Saved palettes", array(p, "saved"), library, str(p, "id"));
  saved->setEditable(true);
  saved->setInsertPolicy(QComboBox::NoInsert);
  saved->lineEdit()->setPlaceholderText("Search saved palettes");
  saved->completer()->setFilterMode(Qt::MatchContains);
  saved->completer()->setCompletionMode(QCompleter::PopupCompletion);
  lib->addWidget(saved);
  lib->addWidget(button("Open", library, [this, saved] {
    send({{"op", "palette_open"}, {"id", saved->currentData().toString()}});
  }));
  lib->addWidget(button("Delete", library, [this, saved] {
    if (QMessageBox::question(this, "Delete palette",
                              "Delete the selected saved palette?") ==
        QMessageBox::Yes)
      send({{"op", "palette_delete"}, {"id", saved->currentData().toString()}});
  }));
  l->addWidget(library);
  auto gen = new QGroupBox("Generate palette", w);
  auto f = new QFormLayout(gen);
  auto generations = combo("Palette generations", array(p, "generations"), gen);
  int generationIndex =
      generations->findData(generationDraft.value("generation").toString());
  if (generationIndex >= 0)
    generations->setCurrentIndex(generationIndex);
  connect(
      generations, &QComboBox::currentIndexChanged, this, [this, generations] {
        generationDraft["generation"] = generations->currentData().toString();
      });
  auto load = button("Load generation", gen, [this, generations] {
    send({{"op", "palette_generation"},
          {"id", generations->currentData().toString()}});
  });
  f->addRow(generations, load);
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
  connect(seed, &QLineEdit::textChanged, this,
          [this](const QString &value) { generationDraft["seed"] = value; });
  f->addRow("Seed", seed);
  auto base = line("Base color (empty for current)",
                   generationDraft.value("base").toString(), gen);
  connect(base, &QLineEdit::textChanged, this,
          [this](const QString &value) { generationDraft["base"] = value; });
  f->addRow("Base color", base);
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
    connect(
        pair.first, &QDoubleSpinBox::valueChanged, this,
        [this, pair](double value) { generationDraft[pair.second] = value; });
  f->addRow("Hue minimum", hmin);
  f->addRow("Hue maximum", hmax);
  f->addRow("Lightness minimum", lmin);
  f->addRow("Lightness maximum", lmax);
  f->addRow("Chroma minimum", cmin);
  f->addRow("Chroma maximum", cmax);
  auto contrast = line("Contrast (optional)",
                       generationDraft.value("contrast").toString(), gen);
  connect(
      contrast, &QLineEdit::textChanged, this,
      [this](const QString &value) { generationDraft["contrast"] = value; });
  f->addRow("Contrast", contrast);
  f->addRow(button("Generate", gen, [=, this] {
    bool ok = false;
    qulonglong sv = seed->text().toULongLong(&ok);
    if (!ok) {
      QMessageBox::warning(this, "Invalid seed",
                           "Enter a non-negative integer seed.");
      return;
    }
    QJsonValue cv = QJsonValue::Null;
    if (!contrast->text().trimmed().isEmpty()) {
      double x = contrast->text().toDouble(&ok);
      if (!ok) {
        QMessageBox::warning(this, "Invalid contrast",
                             "Enter a numeric contrast.");
        return;
      }
      cv = x;
    }
    send({{"op", "palette_generate"},
          {"mode", mode->currentData().toString()},
          {"count", count->value()},
          {"seed", QString::number(sv)},
          {"base", base->text().isEmpty() ? QJsonValue(QJsonValue::Null)
                                          : QJsonValue(base->text())},
          {"hue_min", hmin->value()},
          {"hue_max", hmax->value()},
          {"lightness_min", lmin->value()},
          {"lightness_max", lmax->value()},
          {"chroma_min", cmin->value()},
          {"chroma_max", cmax->value()},
          {"contrast", cv}});
  }));
  l->addWidget(gen);
  auto status = new QLabel(
      str(p, "error").isEmpty() ? str(p, "status") : str(p, "error"), w);
  status->setWordWrap(true);
  l->addWidget(status);
  l->addStretch();
  return w;
}

QWidget *Frontend::themePage(Scene *reuse) {
  auto t = object(state.value("theme")), project = object(t.value("project"));
  auto w = new QWidget;
  auto l = new QVBoxLayout(w);
  auto selection = new QHBoxLayout;
  auto plugins =
      combo("Theme plugins", array(t, "plugins"), w,
            themeDraft.value("plugin").toString(str(project, "plugin_id")));
  connect(plugins, &QComboBox::currentIndexChanged, this, [this, plugins] {
    themeDraft["plugin"] = plugins->currentData().toString();
  });
  selection->addWidget(plugins);
  auto discard = [this, project] {
    return !project.value("dirty").toBool() ||
           QMessageBox::question(this, "Unsaved theme",
                                 "Discard unsaved theme changes?") ==
               QMessageBox::Yes;
  };
  selection->addWidget(button("New theme", w, [this, plugins, discard] {
    if (discard())
      send({{"op", "theme_choose"},
            {"id", plugins->currentData().toString()},
            {"discard", true}});
  }));
  auto saved = combo("Saved themes", array(t, "saved"), w,
                     themeDraft.value("saved").toString());
  connect(saved, &QComboBox::currentIndexChanged, this, [this, saved] {
    themeDraft["saved"] = saved->currentData().toString();
  });
  selection->addWidget(saved);
  selection->addWidget(button("Open theme", w, [this, saved, discard] {
    if (discard())
      send({{"op", "theme_open"},
            {"id", saved->currentData().toString()},
            {"discard", true}});
  }));
  selection->addWidget(
      button("Reload", w, [this] { send({{"op", "theme_reload"}}); }));
  l->addLayout(selection);
  auto name = line("Theme name", str(project, "name"), w);
  l->addWidget(name);
  connect(name, &QLineEdit::editingFinished, this, [this, name] {
    send({{"op", "theme_name"}, {"name", name->text()}});
  });
  auto actions = new QHBoxLayout;
  for (auto pair : QList<QPair<QString, QString>>{
           {"Save", "theme_save"},
           {"Undo", "theme_undo"},
           {"Redo", "theme_redo"},
           {"Use active palette", "theme_palette_bind"},
           {"Render preview", "theme_preview"}}) {
    auto b =
        button(pair.first, w, [this, pair] { send({{"op", pair.second}}); });
    if (pair.second == "theme_undo")
      b->setEnabled(t.value("can_undo").toBool());
    if (pair.second == "theme_redo")
      b->setEnabled(t.value("can_redo").toBool());
    actions->addWidget(b);
  }
  l->addLayout(actions);
  auto scene =
      combo("Preview scene", array(t, "scenes"), w, str(project, "scene"));
  connect(scene, &QComboBox::activated, this, [this, scene](int) {
    send({{"op", "theme_scene"}, {"id", scene->currentData().toString()}});
  });
  l->addWidget(scene);
  if (t.value("scene").isObject()) {
    if (reuse) {
      reuse->setParent(w);
      l->addWidget(reuse);
    } else {
      l->addWidget(new Scene(
          t.value("scene").toObject(), [this](QJsonObject o) { send(o); }, w));
    }
  }
  auto roles = new QGroupBox("Theme roles", w);
  auto rl = new QVBoxLayout(roles);
  for (auto value : array(t, "roles")) {
    auto o = value.toObject();
    auto id = str(o, "id");
    auto row = new QWidget(roles);
    auto r = new QHBoxLayout(row);
    auto sw = new Swatch(
        color(str(o, "color")),
        str(o, "name") + (str(t, "selected") == id ? " (selected)" : ""), row);
    r->addWidget(sw);
    connect(sw, &QPushButton::clicked, this,
            [this, id] { send({{"op", "theme_select"}, {"role", id}}); });
    auto edit = line("Role color", str(o, "value"), row);
    r->addWidget(edit);
    connect(edit, &QLineEdit::editingFinished, this, [this, id, edit] {
      send({{"op", "theme_assign"}, {"role", id}, {"color", edit->text()}});
    });
    r->addWidget(button("Default", row, [this, id] {
      send({{"op", "theme_default"}, {"role", id}});
    }));
    auto link = new QComboBox(row);
    link->setAccessibleName("Palette swatch link");
    link->setEditable(true);
    for (auto v : array(t, "palette_swatches"))
      link->addItem(str(v.toObject(), "name"));
    link->setEditText(str(o, "link"));
    r->addWidget(link);
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
  auto palette = new QGroupBox("Active palette (drag to scene)", w);
  auto pl = new QHBoxLayout(palette);
  for (auto v : array(t, "palette_swatches")) {
    auto o = v.toObject();
    pl->addWidget(new Swatch(color(str(o, "color")), str(o, "name"), palette));
  }
  pl->addStretch();
  l->addWidget(palette);
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
  auto exportRow = new QHBoxLayout;
  auto dir =
      line("Export directory", themeDraft.value("directory").toString(), w);
  connect(dir, &QLineEdit::textChanged, this,
          [this](const QString &value) { themeDraft["directory"] = value; });
  exportRow->addWidget(dir);
  exportRow->addWidget(button("Choose directory", w, [this, dir] {
    auto path =
        QFileDialog::getExistingDirectory(this, "Export theme", dir->text());
    if (!path.isEmpty())
      dir->setText(path);
  }));
  auto overwrite = new QCheckBox("Overwrite existing files", w);
  overwrite->setChecked(themeDraft.value("overwrite").toBool());
  connect(overwrite, &QCheckBox::toggled, this,
          [this](bool value) { themeDraft["overwrite"] = value; });
  exportRow->addWidget(overwrite);
  exportRow->addWidget(button("Export", w, [this, dir, overwrite] {
    if (dir->text().trimmed().isEmpty()) {
      QMessageBox::warning(this, "Export theme", "Choose an export directory.");
      return;
    }
    send({{"op", "theme_export"},
          {"directory", dir->text()},
          {"overwrite", overwrite->isChecked()}});
  }));
  l->addLayout(exportRow);
  auto error = new QLabel(str(t, "error"), w);
  error->setWordWrap(true);
  l->addWidget(error);
  l->addStretch();
  return w;
}

void Frontend::renderConfig() {
  if (configSave) {
    configSave->setEnabled(state.value("load_error").isNull() &&
                           !state.value("busy").toBool());
    configRebind->setEnabled(!state.value("busy").toBool());
    configStatus->setText(message());
    shortcutStatus->setText(str(state, "shortcut_status"));
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
  auto limit =
      line("Stored history limit", str(configDraft, "history_limit"), w);
  limit->setValidator(new QRegularExpressionValidator(
      QRegularExpression("[0-9]{1,20}"), limit));
  f->addRow("Stored history limit", limit);
  connect(limit, &QLineEdit::textChanged, this,
          [this](const QString &v) { configDraft["history_limit"] = v; });
  auto number = [&](const char *key, const QString &label, int min, int max) {
    auto s = spin(label, configDraft.value(key).toInt(), min, max, w);
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
  });
  configSave->setEnabled(state.value("load_error").isNull() &&
                         !state.value("busy").toBool());
  row->addWidget(configSave);
  configRebind =
      button("Rebind shortcuts", w, [this] { send({{"op", "rebind"}}); });
  configRebind->setEnabled(!state.value("busy").toBool());
  row->addWidget(configRebind);
  row->addWidget(button("Close", w, [this] { close(); }));
  l->addLayout(row);
  configStatus = new QLabel(message(), w);
  configStatus->setWordWrap(true);
  configStatus->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
  l->addWidget(configStatus);
  shortcutStatus = new QLabel(str(state, "shortcut_status"), w);
  shortcutStatus->setWordWrap(true);
  shortcutStatus->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Preferred);
  l->addWidget(shortcutStatus);
  l->addStretch();
  replace(scroll(w));
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
