#import <CoreFoundation/CoreFoundation.h>
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>
#import <IOSurface/IOSurface.h>
#import <Metal/Metal.h>
#import <dispatch/dispatch.h>

#include <Metal/Metal.hpp>
#include <mln/actor/scheduler.hpp>
#include <mln/gfx/backend_scope.hpp>
#include <mln/gfx/headless_frontend.hpp>
#include <mln/map/bound_options.hpp>
#include <mln/map/camera.hpp>
#include <mln/map/map.hpp>
#include <mln/map/map_observer.hpp>
#include <mln/map/map_options.hpp>
#include <mln/mtl/context.hpp>
#include <mln/mtl/headless_backend.hpp>
#include <mln/mtl/renderable_resource.hpp>
#include <mln/mtl/texture2d.hpp>
#include <mln/storage/resource_options.hpp>
#include <mln/style/style.hpp>
#include <mln/util/logging.hpp>
#include <mln/util/run_loop.hpp>

#include <algorithm>
#include <atomic>
#include <cstdint>
#include <memory>
#include <string>

extern "C" {
typedef void (*MlnFrameCallback)(void* user_ctx, void* cv_pixel_buffer);
typedef void (*MlnErrorCallback)(void* user_ctx, const char* error_msg);
}

namespace {

struct QuietLogObserver final : public mln::Log::Observer {
    bool onRecord(mln::EventSeverity severity, mln::Event, int64_t, const std::string&) override {
        return severity == mln::EventSeverity::Debug || severity == mln::EventSeverity::Info;
    }
};

__attribute__((constructor, used))
static void installQuietLogObserver() {
    mln::Log::setObserver(std::make_unique<QuietLogObserver>());
}

static void noopSourcePerform(void*) {}

struct ContinuousObserver final : public mln::MapObserver {
    MlnErrorCallback onError = nullptr;
    void* userCtx = nullptr;
    bool hadError = false;

    void clearError() {
        if (hadError) {
            hadError = false;
            if (onError) onError(userCtx, nullptr);
        }
    }
    void onDidFinishLoadingMap() override { clearError(); }
    void onDidFinishRenderingFrame(const RenderFrameStatus&) override { clearError(); }
    void onDidFailLoadingMap(mln::MapLoadError, const std::string& msg) override {
        hadError = true;
        if (onError) onError(userCtx, msg.c_str());
    }
};

struct ContinuousFrontend final : public mln::HeadlessFrontend {
    std::atomic<bool> frameRequested{true};
    CFRunLoopRef ownerRunLoop = nullptr;
    CFRunLoopSourceRef wakeSource = nullptr;

    ContinuousFrontend(mln::Size size, float pixelRatio)
        : mln::HeadlessFrontend(size,
                                pixelRatio,
                                mln::gfx::HeadlessBackend::SwapBehaviour::Flush,
                                mln::gfx::ContextMode::Unique,
                                std::nullopt,
                                false) {}

    void update(std::shared_ptr<mln::UpdateParameters> params) override {
        mln::HeadlessFrontend::update(std::move(params));
        frameRequested.store(true, std::memory_order_release);
        if (wakeSource) CFRunLoopSourceSignal(wakeSource);
        if (ownerRunLoop) CFRunLoopWakeUp(ownerRunLoop);
    }
};

mln::gfx::Texture2DPtr makeAttachment(mln::mtl::Context& ctx,
                                      mln::Size size,
                                      mln::gfx::TexturePixelType pixelType,
                                      mln::gfx::TextureChannelDataType dataType) {
    auto tex = ctx.createTexture2D();
    tex->setSize(size);
    tex->setFormat(pixelType, dataType);
    tex->setSamplerConfiguration({.filter = mln::gfx::TextureFilterType::Linear,
                                  .wrapU = mln::gfx::TextureWrapType::Clamp,
                                  .wrapV = mln::gfx::TextureWrapType::Clamp});
    static_cast<mln::mtl::Texture2D*>(tex.get())
        ->setUsage(MTL::TextureUsageShaderRead | MTL::TextureUsageShaderWrite | MTL::TextureUsageRenderTarget);
    return tex;
}

/// Renders MapLibre Metal directly into a 4-slot IOSurface-backed CVPixelBuffer ring
/// with zero intermediate offscreen texture, zero blit pass, and non-blocking async commit.
class DirectIOSurfaceResource final : public mln::mtl::RenderableResource {
public:
    static constexpr int kRingSize = 4;

    DirectIOSurfaceResource(mln::mtl::Context& ctx, mln::Size sz, MlnFrameCallback cb, void* user)
        : context(ctx), size(sz), onFrame(cb), userCtx(user) {
        auto device = (__bridge id<MTLDevice>)(void*)context.getBackend().getDevice().get();
        CVMetalTextureCacheCreate(kCFAllocatorDefault, nullptr, device, nullptr, &texCache);
        ringSem = dispatch_semaphore_create(kRingSize);
        for (int i = 0; i < kRingSize; ++i) allocateSlot(i);
        depthTexture = makeAttachment(context, size, mln::gfx::TexturePixelType::Depth, mln::gfx::TextureChannelDataType::Float);
        stencilTexture = makeAttachment(context, size, mln::gfx::TexturePixelType::Stencil, mln::gfx::TextureChannelDataType::UnsignedByte);
    }

    ~DirectIOSurfaceResource() noexcept override {
        shuttingDown.store(true, std::memory_order_release);
        drainGpu();
        commandBuffer.reset();
        renderPassDescriptor.reset();
        depthTexture.reset();
        stencilTexture.reset();
        for (int i = 0; i < kRingSize; ++i) releaseSlot(i);
        if (texCache) CFRelease(texCache);
    }

    void bind() override {
        if (commandBuffer) return; // UploadPass + RenderPass share one command buffer per frame.
        dispatch_semaphore_wait(ringSem, DISPATCH_TIME_FOREVER);

        int chosen = nextSlot;
        for (int i = 0; i < kRingSize; ++i) {
            int idx = (nextSlot + i) % kRingSize;
            if (ringBuffers[idx] && CFGetRetainCount(ringBuffers[idx]) == 1) {
                chosen = idx;
                break;
            }
        }
        activeSlot = chosen;
        nextSlot = (chosen + 1) % kRingSize;
        if (!ringBuffers[activeSlot] || CFGetRetainCount(ringBuffers[activeSlot]) > 1) {
            releaseSlot(activeSlot);
            allocateSlot(activeSlot);
        }

        commandBuffer = NS::RetainPtr(context.getBackend().getCommandQueue()->commandBuffer());
        renderPassDescriptor = NS::TransferPtr(MTL::RenderPassDescriptor::alloc()->init());
        if (auto* color = renderPassDescriptor->colorAttachments()->object(0)) {
            id<MTLTexture> tex = CVMetalTextureGetTexture(ringMetalTex[activeSlot]);
            color->setTexture(reinterpret_cast<MTL::Texture*>((__bridge void*)tex));
        }
        depthTexture->create();
        if (auto* depth = renderPassDescriptor->depthAttachment()) {
            depth->setTexture(static_cast<mln::mtl::Texture2D*>(depthTexture.get())->getMetalTexture());
        }
        stencilTexture->create();
        if (auto* stencil = renderPassDescriptor->stencilAttachment()) {
            stencil->setTexture(static_cast<mln::mtl::Texture2D*>(stencilTexture.get())->getMetalTexture());
        }
    }

    void swap() override {
        if (!commandBuffer) return;
        CVPixelBufferRef buf = ringBuffers[activeSlot];
        if (buf) CVPixelBufferRetain(buf);
        dispatch_semaphore_t sem = ringSem;
        MlnFrameCallback cb = onFrame;
        void* ctx = userCtx;
        auto* shuttingDownPtr = &shuttingDown;

        id<MTLCommandBuffer> objcCmd = (__bridge id<MTLCommandBuffer>)(void*)commandBuffer.get();
        [objcCmd addCompletedHandler:^(id<MTLCommandBuffer> _Nonnull done) {
            if (buf) {
                if (done.status == MTLCommandBufferStatusCompleted &&
                    !shuttingDownPtr->load(std::memory_order_acquire) && cb) {
                    cb(ctx, (void*)buf);
                }
                CVPixelBufferRelease(buf);
            }
            dispatch_semaphore_signal(sem);
        }];
        [objcCmd commit];
        commandBuffer.reset();
        renderPassDescriptor.reset();
    }

    void drainGpu() {
        for (int i = 0; i < kRingSize; ++i) dispatch_semaphore_wait(ringSem, DISPATCH_TIME_FOREVER);
        for (int i = 0; i < kRingSize; ++i) dispatch_semaphore_signal(ringSem);
    }

    const mln::mtl::RendererBackend& getBackend() const override { return context.getBackend(); }
    const mln::mtl::MTLCommandBufferPtr& getCommandBuffer() const override { return commandBuffer; }
    mln::mtl::MTLBlitPassDescriptorPtr getUploadPassDescriptor() const override {
        return NS::TransferPtr(MTL::BlitPassDescriptor::alloc()->init());
    }
    const mln::mtl::MTLRenderPassDescriptorPtr& getRenderPassDescriptor() const override {
        return renderPassDescriptor;
    }

private:
    void allocateSlot(int idx) {
        NSDictionary* attrs = @{
            (NSString*)kCVPixelBufferIOSurfacePropertiesKey : @{},
            (NSString*)kCVPixelBufferMetalCompatibilityKey : @YES,
        };
        CVPixelBufferCreate(kCFAllocatorDefault, size.width, size.height, kCVPixelFormatType_32BGRA,
                            (__bridge CFDictionaryRef)attrs, &ringBuffers[idx]);
        if (ringBuffers[idx]) {
            CVMetalTextureCacheCreateTextureFromImage(
                kCFAllocatorDefault, texCache, ringBuffers[idx], nullptr,
                MTLPixelFormatBGRA8Unorm, size.width, size.height, 0, &ringMetalTex[idx]);
        }
    }

    void releaseSlot(int idx) {
        if (ringMetalTex[idx]) { CFRelease(ringMetalTex[idx]); ringMetalTex[idx] = nullptr; }
        if (ringBuffers[idx]) { CVPixelBufferRelease(ringBuffers[idx]); ringBuffers[idx] = nullptr; }
    }

    mln::mtl::Context& context;
    mln::Size size;
    MlnFrameCallback onFrame = nullptr;
    void* userCtx = nullptr;
    CVMetalTextureCacheRef texCache = nullptr;
    dispatch_semaphore_t ringSem = nullptr;
    CVPixelBufferRef ringBuffers[kRingSize] = {};
    CVMetalTextureRef ringMetalTex[kRingSize] = {};
    int nextSlot = 0;
    int activeSlot = 0;
    std::atomic<bool> shuttingDown{false};

    mln::gfx::Texture2DPtr depthTexture;
    mln::gfx::Texture2DPtr stencilTexture;
    mln::mtl::MTLCommandBufferPtr commandBuffer;
    mln::mtl::MTLRenderPassDescriptorPtr renderPassDescriptor;
};

struct MlnMetalHost {
    std::unique_ptr<mln::util::RunLoop> runLoop;
    std::unique_ptr<ContinuousObserver> observer;
    std::unique_ptr<ContinuousFrontend> frontend;
    std::unique_ptr<mln::Map> map;
    DirectIOSurfaceResource* directRes = nullptr;
    CFRunLoopRef cfRunLoop = nullptr;
    CFRunLoopSourceRef wakeSource = nullptr;
    MlnFrameCallback onFrame = nullptr;
    void* userCtx = nullptr;
    float pixelRatio = 2.0f;

    void installDirectResource(uint32_t w, uint32_t h) {
        auto* backend = static_cast<mln::mtl::HeadlessBackend*>(frontend->getBackend());
        mln::gfx::BackendScope scope{*backend};
        auto& ctx = static_cast<mln::mtl::Context&>(backend->getContext());
        mln::Size phys{
            std::max<uint32_t>(1, static_cast<uint32_t>(w * pixelRatio)),
            std::max<uint32_t>(1, static_cast<uint32_t>(h * pixelRatio)),
        };
        auto direct = std::make_unique<DirectIOSurfaceResource>(ctx, phys, onFrame, userCtx);
        directRes = direct.get();
        backend->setResource(std::move(direct));
    }

    ~MlnMetalHost() {
        if (directRes) directRes->drainGpu();
        map.reset();
        frontend.reset();
        directRes = nullptr;
        observer.reset();
        runLoop.reset();
        if (wakeSource) {
            if (cfRunLoop) CFRunLoopRemoveSource(cfRunLoop, wakeSource, kCFRunLoopDefaultMode);
            CFRunLoopSourceInvalidate(wakeSource);
            CFRelease(wakeSource);
        }
        if (cfRunLoop) CFRelease(cfRunLoop);
    }
};

} // namespace

extern "C" {

void* moss_mln_create(uint32_t logical_w,
                      uint32_t logical_h,
                      float pixel_ratio,
                      const char* cache_path,
                      uint64_t max_cache_bytes,
                      const char* style_json,
                      double init_lat,
                      double init_lng,
                      double init_zoom,
                      MlnFrameCallback on_frame,
                      MlnErrorCallback on_error,
                      void* user_ctx) {
    @autoreleasepool {
        auto* host = new MlnMetalHost();
        host->cfRunLoop = (CFRunLoopRef)CFRetain(CFRunLoopGetCurrent());
        CFRunLoopSourceContext srcCtx = {.perform = noopSourcePerform};
        host->wakeSource = CFRunLoopSourceCreate(kCFAllocatorDefault, 0, &srcCtx);
        if (host->wakeSource) CFRunLoopAddSource(host->cfRunLoop, host->wakeSource, kCFRunLoopDefaultMode);
        host->onFrame = on_frame;
        host->userCtx = user_ctx;
        host->pixelRatio = std::clamp(pixel_ratio, 1.0f, 3.0f);

        host->runLoop = std::make_unique<mln::util::RunLoop>(mln::util::RunLoop::Type::New);
        mln::Scheduler::SetCurrent(host->runLoop.get());
        host->observer = std::make_unique<ContinuousObserver>();
        host->observer->onError = on_error;
        host->observer->userCtx = user_ctx;

        mln::Size sz{std::max<uint32_t>(1, logical_w), std::max<uint32_t>(1, logical_h)};
        host->frontend = std::make_unique<ContinuousFrontend>(sz, host->pixelRatio);
        host->frontend->ownerRunLoop = host->cfRunLoop;
        host->frontend->wakeSource = host->wakeSource;
        host->installDirectResource(sz.width, sz.height);

        mln::MapOptions mapOpts;
        mapOpts.withMapMode(mln::MapMode::Continuous).withSize(sz).withPixelRatio(host->pixelRatio);
        mln::ResourceOptions resOpts;
        if (cache_path && cache_path[0]) resOpts.withCachePath(cache_path);
        if (max_cache_bytes > 0) resOpts.withMaximumCacheSize(max_cache_bytes);

        host->map = std::make_unique<mln::Map>(*host->frontend, *host->observer, mapOpts, resOpts);
        host->map->setBounds(mln::BoundOptions().withMinZoom(0.0).withMaxZoom(18.0));
        host->map->jumpTo(mln::CameraOptions().withCenter(mln::LatLng{init_lat, init_lng}).withZoom(init_zoom));
        if (style_json && style_json[0]) host->map->getStyle().loadJSON(style_json);
        return host;
    }
}

void moss_mln_set_camera(void* ptr, double lat, double lng, double zoom) {
    @autoreleasepool {
        auto* host = static_cast<MlnMetalHost*>(ptr);
        mln::Scheduler::SetCurrent(host->runLoop.get());
        host->map->jumpTo(mln::CameraOptions().withCenter(mln::LatLng{lat, lng}).withZoom(zoom));
        host->frontend->frameRequested.store(true, std::memory_order_release);
    }
}

void moss_mln_resize(void* ptr, uint32_t logical_w, uint32_t logical_h) {
    @autoreleasepool {
        auto* host = static_cast<MlnMetalHost*>(ptr);
        mln::Scheduler::SetCurrent(host->runLoop.get());
        mln::Size sz{std::max<uint32_t>(1, logical_w), std::max<uint32_t>(1, logical_h)};
        if (host->directRes) host->directRes->drainGpu();
        host->frontend->setSize(sz);
        host->map->setSize(sz);
        host->installDirectResource(sz.width, sz.height);
        host->frontend->frameRequested.store(true, std::memory_order_release);
    }
}

void moss_mln_set_style_json(void* ptr, const char* style_json) {
    @autoreleasepool {
        auto* host = static_cast<MlnMetalHost*>(ptr);
        mln::Scheduler::SetCurrent(host->runLoop.get());
        if (style_json && style_json[0]) {
            host->map->getStyle().loadJSON(style_json);
            host->frontend->frameRequested.store(true, std::memory_order_release);
        }
    }
}

void moss_mln_pump(void* ptr, double timeout_sec) {
    @autoreleasepool {
        auto* host = static_cast<MlnMetalHost*>(ptr);
        mln::Scheduler::SetCurrent(host->runLoop.get());
        double wait = host->frontend->frameRequested.load(std::memory_order_acquire) ? 0.0 : timeout_sec;
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, wait, true);
        host->runLoop->runOnce();
        if (host->frontend->frameRequested.exchange(false, std::memory_order_acq_rel)) {
            host->frontend->renderFrame();
            if (host->directRes && host->directRes->getCommandBuffer()) host->directRes->swap();
        }
    }
}

void moss_mln_wake(void* ptr) {
    auto* host = static_cast<MlnMetalHost*>(ptr);
    if (!host) return;
    if (host->wakeSource) CFRunLoopSourceSignal(host->wakeSource);
    if (host->cfRunLoop) CFRunLoopWakeUp(host->cfRunLoop);
}

void moss_mln_destroy(void* ptr) {
    @autoreleasepool { delete static_cast<MlnMetalHost*>(ptr); }
}

} // extern "C"
