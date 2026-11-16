// SPDX-License-Identifier: GPL-2.0-or-later
// This component links to KWin's GPL-licensed compositor APIs.
#include "compositor.h"
#include "core/inputdevice.h"
#include "core/output.h"
#include "core/outputlayer.h"
#include "core/renderdevice.h"
#include "core/renderviewport.h"
#include "effect/effect.h"
#include "effect/effecthandler.h"
#include "input_event.h"
#include "main.h"
#include "opengl/eglcontext.h"
#include "opengl/glutils.h"
#include "scene/scene.h"
#include "scene/workspacescene.h"
#include "workspace.h"

#include <QDBusConnection>
#include <QDBusContext>
#include <QDBusMessage>
#include <QDBusServiceWatcher>
#include <QImage>
#include <QKeyEvent>
#include <QPainter>
#include <QPointer>
#include <QTimer>
#include <algorithm>
#include <cmath>
#include <memory>

using namespace KWin;

namespace {
constexpr int MaximumSampleSize = 33;
constexpr auto ObjectPath = "/org/apartment38/SwatchBoard/Picker";
constexpr auto ErrorPrefix = "org.apartment38.SwatchBoard.Picker.Error.";

class CaptureLayer final : public OutputLayer
{
public:
    CaptureLayer(LogicalOutput *output, GLFramebuffer *framebuffer)
        : OutputLayer(output->backendOutput(), OutputLayerType::Primary), m_framebuffer(framebuffer) {}
    FormatModifierMap supportedDrmFormats() const override { return {}; }
    void releaseBuffers() override {}
    std::optional<OutputLayerBeginFrameInfo> beginFrame(OutputFrame * = nullptr) override
    {
        if (!m_framebuffer->context()->makeCurrent()) return std::nullopt;
        return OutputLayerBeginFrameInfo{RenderTarget(m_framebuffer), Region::infinite()};
    }
    bool endFrame(const Region &, const Region &, OutputFrame *) override { return true; }
private:
    GLFramebuffer *m_framebuffer;
};
}

class PickerEffect final : public Effect, protected QDBusContext
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.apartment38.SwatchBoard.PickerEffect1")
    Q_PROPERTY(bool active READ isActive)
    Q_PROPERTY(QString currentHex READ currentHex)
    Q_PROPERTY(uint zoom READ zoom)
    Q_PROPERTY(QPointF samplePosition READ samplePosition)
public:
    PickerEffect()
        : m_watcher(this)
    {
        auto bus = QDBusConnection::sessionBus();
        m_registered = bus.registerObject(QString::fromLatin1(ObjectPath), this,
                                           QDBusConnection::ExportScriptableSlots | QDBusConnection::ExportScriptableProperties);
        m_watcher.setConnection(bus);
        m_watcher.setWatchMode(QDBusServiceWatcher::WatchForUnregistration);
        connect(&m_watcher, &QDBusServiceWatcher::serviceUnregistered, this, [this](const QString &name) {
            if (m_active && name == m_client) finish("Cancelled", "The requesting client disconnected");
        });
        m_timer.setInterval(33);
        connect(&m_timer, &QTimer::timeout, this, [this] { capture(); });
        connect(effects, &EffectsHandler::screenAboutToLock, this, [this] { if (m_active) finish("Cancelled", "Screen locked"); });
        connect(effects, &EffectsHandler::screenLockingChanged, this, [this](bool locked) { if (locked && m_active) finish("Cancelled", "Screen locked"); });
        connect(effects, &EffectsHandler::screenRemoved, this, [this](LogicalOutput *output) { if (m_active && output == m_output) finish("Cancelled", "Output removed"); });
        connect(effects, &EffectsHandler::activeFullScreenEffectChanged, this, [this] {
            if (m_active && effects->activeFullScreenEffect() != this) finish("Cancelled", "Input ownership lost");
        });
    }
    ~PickerEffect() override
    {
        if (m_active) finish("Cancelled", "Picker unloaded");
        if (m_registered) QDBusConnection::sessionBus().unregisterObject(QString::fromLatin1(ObjectPath));
        if (effects->makeOpenGLContextCurrent()) clearCapture();
    }
    bool isActive() const override { return m_active; }
    QString currentHex() const { return m_valid ? m_color.name().toUpper() : QString(); }
    uint zoom() const { return m_zoom; }
    QPointF samplePosition() const { return m_position; }
    int requestedEffectChainPosition() const override { return 99; }

    Q_SCRIPTABLE Q_SLOT uint pick(uint zoom)
    {
        if (zoom < 1 || zoom > 32) { sendErrorReply(QString::fromLatin1(ErrorPrefix) + "InvalidZoom", "Zoom must be between 1 and 32"); return 0; }
        if (m_active || effects->activeFullScreenEffect() || effects->isMouseInterception() || effects->hasKeyboardGrab() || effects->isScreenLocked()) {
            sendErrorReply(QString::fromLatin1(ErrorPrefix) + "Busy", "Input is unavailable"); return 0;
        }
        if (!effects->isOpenGLCompositing() || !workspace()->outputAt(effects->cursorPos()) || !effects->grabKeyboard(this)) {
            sendErrorReply(QString::fromLatin1(ErrorPrefix) + "Unavailable", "Picker input or rendering is unavailable"); return 0;
        }
        m_client = message().service();
        m_reply = message();
        setDelayedReply(true);
        m_zoom = zoom;
        m_active = true;
        m_valid = false;
        m_watcher.addWatchedService(m_client);
        effects->setActiveFullScreenEffect(this);
        effects->startMouseInterception(this, Qt::BlankCursor);
        effects->hideCursor();
        m_timer.start();
        capture();
        return 0;
    }
    Q_SCRIPTABLE Q_SLOT void cancel()
    {
        if (m_active && message().service() == m_client) finish("Cancelled", "Picker cancelled");
    }
    void pointerButton(PointerButtonEvent *event) override
    {
        if (!m_active || event->state != PointerButtonState::Released) return;
        if (event->button == Qt::RightButton) finish("Cancelled", "Picker cancelled");
        else if (event->button == Qt::LeftButton) select();
    }
    void grabbedKeyboardEvent(QKeyEvent *event) override
    {
        if (!m_active || event->type() != QEvent::KeyRelease || event->isAutoRepeat()) return;
        if (event->key() == Qt::Key_Escape) finish("Cancelled", "Picker cancelled");
        else if (event->key() == Qt::Key_Return || event->key() == Qt::Key_Enter) select();
    }
    bool paintScreen(const RenderTarget &target, const RenderViewport &viewport, int mask, const Region &region, LogicalOutput *screen) override
    {
        if (!effects->paintScreen(target, viewport, mask, region, screen)) return false;
        if (!m_active || m_capturing || !m_valid || screen != m_output || !m_hud) return true;
        const qreal scale = viewport.scale();
        auto shader = ShaderManager::instance()->pushShader(ShaderTrait::MapTexture);
        QMatrix4x4 matrix = viewport.projectionMatrix();
        matrix.translate(m_hudPosition.x() * scale, m_hudPosition.y() * scale);
        shader->setUniform(GLShader::Mat4Uniform::ModelViewProjectionMatrix, matrix);
        glEnable(GL_BLEND);
        glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
        m_hud->render(QSizeF(m_hudWidth * scale, m_hudHeight * scale));
        if (m_crosshair) {
            QMatrix4x4 cursorMatrix = viewport.projectionMatrix();
            const qreal pixelScale = m_output->scale();
            cursorMatrix.translate((m_position.x() - 12.5 / pixelScale) * scale, (m_position.y() - 12.5 / pixelScale) * scale);
            shader->setUniform(GLShader::Mat4Uniform::ModelViewProjectionMatrix, cursorMatrix);
            m_crosshair->render(QSizeF(25 * scale / pixelScale, 25 * scale / pixelScale));
        }
        glDisable(GL_BLEND);
        ShaderManager::instance()->popShader();
        return true;
    }
private:
    void clearCapture()
    {
        m_cursorView.reset(); m_view.reset(); m_layer.reset(); m_framebuffer.reset(); m_texture.reset(); m_hud.reset(); m_crosshair.reset();
        m_output = nullptr;
        m_sampleImage = QImage();
        m_hudImage = QImage();
    }
    bool prepare(LogicalOutput *output)
    {
        const auto bounds = output->geometryF();
        const qreal scale = output->scale();
        const int available = int(std::min(bounds.width() * scale - 12 * scale, bounds.height() * scale - 49 * scale));
        int sampleSize = std::clamp(available / int(m_zoom), 1, MaximumSampleSize);
        if ((sampleSize % 2) == 0) --sampleSize;
        if (m_output == output && m_view && m_sampleSize == sampleSize) return true;
        clearCapture();
        auto device = Compositor::self()->primaryDevice();
        if (!device || !device->eglContext() || !device->eglContext()->makeCurrent() || !kwinApp()->scene()) return false;
        m_sampleSize = sampleSize;
        m_sampleImage = QImage(sampleSize, sampleSize, QImage::Format_RGBA8888_Premultiplied);
        if (m_sampleImage.isNull()) return false;
        m_texture = GLTexture::allocate(GL_RGB8, QSize(sampleSize, sampleSize));
        if (!m_texture) return false;
        m_texture->setFilter(GL_NEAREST);
        m_texture->setWrapMode(GL_CLAMP_TO_EDGE);
        m_framebuffer = std::make_unique<GLFramebuffer>(m_texture.get());
        if (!m_framebuffer->valid()) return false;
        m_layer = std::make_unique<CaptureLayer>(output, m_framebuffer.get());
        m_view = std::make_unique<SceneView>(kwinApp()->scene(), output, nullptr, m_layer.get(), device);
        m_cursorView = std::make_unique<ItemTreeView>(m_view.get(), kwinApp()->scene()->cursorItem(), output, nullptr, nullptr);
        m_cursorView->setExclusive(true);
        m_output = output;
        return true;
    }
    void capture()
    {
        if (!m_active) return;
        const QPointF pos = effects->cursorPos();
        effects->addRepaint(m_dirty);
        LogicalOutput *output = workspace()->outputAt(pos);
        if (!output || effects->isScreenLocked() || !effects->makeOpenGLContextCurrent() || !prepare(output)) { finish("Unavailable", "Unable to capture the current output"); return; }
        const qreal scale = output->scale();
        const auto bounds = output->geometryF();
        const qreal px = std::floor((pos.x() - bounds.x()) * scale);
        const qreal py = std::floor((pos.y() - bounds.y()) * scale);
        m_view->setScale(scale);
        m_view->setViewport(RectF(bounds.x() + (px - m_sampleSize / 2) / scale, bounds.y() + (py - m_sampleSize / 2) / scale, m_sampleSize / scale, m_sampleSize / scale));
        m_view->setNextPresentationTimestamp(std::chrono::steady_clock::now().time_since_epoch(), output->refreshRate(), nullptr);
        m_view->prePaint();
        auto frame = m_layer->beginFrame();
        if (!frame) { m_view->postPaint(); finish("Unavailable", "Unable to render the sample"); return; }
        const Rect damage(QPoint(), QSize(m_sampleSize, m_sampleSize));
        GLFramebuffer::pushFramebuffer(m_framebuffer.get());
        glClearColor(0, 0, 0, 1);
        glClear(GL_COLOR_BUFFER_BIT);
        GLFramebuffer::popFramebuffer();
        m_capturing = true;
        m_view->paint(frame->renderTarget, QPoint(), damage);
        m_capturing = false;
        m_layer->endFrame(damage, damage, nullptr);
        m_view->postPaint();
        GLFramebuffer::pushFramebuffer(m_framebuffer.get());
        (void)glGetError();
        Compositor::self()->primaryDevice()->eglContext()->glReadnPixels(0, 0, m_sampleSize, m_sampleSize, GL_RGBA, GL_UNSIGNED_BYTE, m_sampleImage.sizeInBytes(), m_sampleImage.bits());
        const GLenum error = glGetError();
        GLFramebuffer::popFramebuffer();
        if (error != GL_NO_ERROR) { finish("Unavailable", "Unable to read the sample"); return; }
        m_color = m_sampleImage.pixelColor(m_sampleSize / 2, m_sampleSize / 2);
        if (m_color.alpha() != 255) { finish("Unavailable", "The sample was not fully rendered"); return; }
        m_color.setAlpha(255);
        m_position = QPointF(bounds.x() + (px + 0.5) / scale, bounds.y() + (py + 0.5) / scale);
        m_valid = true;
        drawHud(m_sampleImage, bounds, scale);
        if (!m_hud) { finish("Unavailable", "Unable to draw the picker"); return; }
        effects->addRepaint(m_dirty);
    }
    void drawHud(const QImage &sample, const RectF &bounds, qreal scale)
    {
        const qreal cell = m_zoom / scale;
        const qreal side = m_sampleSize * cell;
        const qreal width = std::max(side + 12, 148.0);
        const qreal height = side + 49;
        const QSize pixels(qCeil(width * scale), qCeil(height * scale));
        if (m_hudImage.size() != pixels) m_hudImage = QImage(pixels, QImage::Format_RGBA8888_Premultiplied);
        m_hudImage.setDevicePixelRatio(scale);
        m_hudImage.fill(Qt::transparent);
        QPainter p(&m_hudImage);
        p.fillRect(QRectF(0, 0, width, height), QColor(20, 20, 20, 245));
        p.setRenderHint(QPainter::SmoothPixmapTransform, false);
        const qreal left = (width - side) / 2;
        // glReadPixels starts at the lower left; flip while drawing, not by
        // allocating and copying another sample image every frame.
        p.save();
        p.translate(left, 5 + side);
        p.scale(cell, -cell);
        p.drawImage(QPointF(0, 0), sample);
        p.restore();
        const qreal cx = left + (m_sampleSize / 2) * cell;
        const qreal cy = 5 + (m_sampleSize / 2) * cell;
        const auto reticle = [&] {
            p.drawRect(QRectF(cx, cy, cell, cell));
            p.drawLine(QPointF(cx - 9, cy + cell / 2), QPointF(cx - 3, cy + cell / 2));
            p.drawLine(QPointF(cx + cell + 3, cy + cell / 2), QPointF(cx + cell + 9, cy + cell / 2));
            p.drawLine(QPointF(cx + cell / 2, cy - 9), QPointF(cx + cell / 2, cy - 3));
            p.drawLine(QPointF(cx + cell / 2, cy + cell + 3), QPointF(cx + cell / 2, cy + cell + 9));
        };
        p.setPen(QPen(Qt::black, 3 / scale));
        reticle();
        p.setPen(QPen(Qt::white, 1 / scale));
        reticle();
        p.fillRect(QRectF(8, side + 14, 28, 26), m_color);
        p.setPen(Qt::white);
        p.drawRect(QRectF(8, side + 14, 28, 26));
        QFont font = p.font(); font.setBold(true); font.setPixelSize(17); p.setFont(font);
        p.drawText(QRectF(44, side + 12, width - 48, 30), Qt::AlignVCenter | Qt::AlignLeft, currentHex());
        p.end();
        if (m_hud && m_hud->size() == pixels) {
            m_hud->update(m_hudImage, Region(Rect(QPoint(), pixels)));
        } else {
            m_hud = GLTexture::upload(m_hudImage);
            if (m_hud) m_hud->setFilter(GL_NEAREST);
        }
        if (!m_crosshair) {
            QImage cursor(25, 25, QImage::Format_RGBA8888_Premultiplied);
            cursor.fill(Qt::transparent);
            QPainter cp(&cursor);
            cp.setPen(QPen(Qt::black, 3));
            cp.drawLine(0, 12, 7, 12); cp.drawLine(17, 12, 24, 12);
            cp.drawLine(12, 0, 12, 7); cp.drawLine(12, 17, 12, 24);
            cp.setPen(QPen(Qt::white, 1));
            cp.drawLine(0, 12, 7, 12); cp.drawLine(17, 12, 24, 12);
            cp.drawLine(12, 0, 12, 7); cp.drawLine(12, 17, 12, 24);
            cp.setPen(QPen(Qt::black, 1));
            cp.drawRect(10, 10, 4, 4);
            cp.setPen(QPen(Qt::white, 1));
            cp.drawRect(11, 11, 2, 2);
            cp.end();
            m_crosshair = GLTexture::upload(cursor);
        }
        const qreal fit = std::min({1.0, bounds.width() / width, bounds.height() / height});
        m_hudWidth = width * fit; m_hudHeight = height * fit;
        m_hudPosition = QPointF(std::clamp(m_position.x() + 18.0, bounds.x(), bounds.right() - m_hudWidth),
                                std::clamp(m_position.y() + 18.0, bounds.y(), bounds.bottom() - m_hudHeight));
        m_dirty = RectF(m_hudPosition, QSizeF(m_hudWidth, m_hudHeight)).toAlignedRect()
            .united(RectF(m_position - QPointF(13 / scale, 13 / scale), QSizeF(26 / scale, 26 / scale)).toAlignedRect());
    }
    void select()
    {
        capture();
        if (m_active && m_valid) finish(nullptr, nullptr);
    }
    void finish(const char *error, const char *text)
    {
        if (!m_active) return;
        m_active = false;
        m_timer.stop();
        m_watcher.removeWatchedService(m_client);
        effects->stopMouseInterception(this);
        effects->ungrabKeyboard();
        effects->showCursor();
        if (effects->activeFullScreenEffect() == this) effects->setActiveFullScreenEffect(nullptr);
        effects->addRepaint(m_dirty);
        m_dirty = Rect();
        auto reply = error ? m_reply.createErrorReply(QString::fromLatin1(ErrorPrefix) + QString::fromLatin1(error), QString::fromLatin1(text))
                           : m_reply.createReply(QVariant::fromValue(uint(m_color.rgba())));
        QDBusConnection::sessionBus().send(reply);
        m_client.clear();
        m_valid = false;
        if (effects->makeOpenGLContextCurrent()) clearCapture();
    }
    bool m_registered = false, m_active = false, m_valid = false, m_capturing = false;
    uint m_zoom = 4;
    int m_sampleSize = MaximumSampleSize;
    QImage m_sampleImage, m_hudImage;
    Rect m_dirty;
    QColor m_color;
    QPointF m_position, m_hudPosition;
    qreal m_hudWidth = 0, m_hudHeight = 0;
    QString m_client;
    QDBusMessage m_reply;
    QDBusServiceWatcher m_watcher;
    QTimer m_timer;
    QPointer<LogicalOutput> m_output;
    std::unique_ptr<GLTexture> m_texture, m_hud, m_crosshair;
    std::unique_ptr<GLFramebuffer> m_framebuffer;
    std::unique_ptr<CaptureLayer> m_layer;
    std::unique_ptr<SceneView> m_view;
    std::unique_ptr<ItemTreeView> m_cursorView;
};

KWIN_EFFECT_FACTORY_SUPPORTED_ENABLED(PickerEffect, "swatchboardpicker.json", return effects->isOpenGLCompositing();, return false;)
#include "picker.moc"
