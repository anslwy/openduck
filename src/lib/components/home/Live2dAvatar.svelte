<script lang="ts">
    import { untrack } from "svelte";
    import { loadStoredContactCubismModelZip } from "$lib/openduck/contacts";
    import type { CubismModelConfig } from "$lib/openduck/types";

    let {
        cubismModel,
        fallbackImageStyle,
        calling,
        speaking,
        expression = null,
        lipSyncValue = null,
    } = $props<{
        cubismModel: CubismModelConfig;
        fallbackImageStyle: string;
        calling: boolean;
        speaking: boolean;
        expression?: string | null;
        lipSyncValue?: number | null;
    }>();

    const CUBISM_CORE_URL = "/live2d/live2dcubismcore.min.js";
    const scriptPromises = new Map<string, Promise<void>>();
    let zipLoaderConfigured = false;

    let containerEl: HTMLDivElement | null = $state(null);
    let canvasEl: HTMLCanvasElement | null = $state(null);
    let loadState = $state<"loading" | "ready" | "error">("loading");
    let app: any = null;
    let model = $state<any>(null);
    let resizeObserver: ResizeObserver | null = null;
    let detachMotionLoopPatch: (() => void) | null = null;
    let loadToken = 0;

    function clamp01(value: number) {
        return Math.min(1, Math.max(0, value));
    }

    function setModelMouthOpen(targetModel: any, value: number) {
        targetModel?.internalModel?.coreModel?.setParameterValueById?.(
            "ParamMouthOpenY",
            clamp01(value),
        );
    }

    function configureLoopingCubismMotions(targetModel: any) {
        const motionManager = targetModel?.internalModel?.motionManager;
        if (!motionManager) {
            return null;
        }

        const patchMotion = (
            group: string,
            index: number,
            motion: any,
        ) => {
            const definition = motionManager.definitions?.[group]?.[index];
            // pixi-live2d-display parses Cubism 4 Meta.Loop but does not apply it.
            const shouldLoop = Boolean(
                motion?._motionData?.loop ?? definition?.Loop,
            );
            if (!shouldLoop || typeof motion?.setIsLoop !== "function") {
                return;
            }

            motion.setIsLoop(true);
            motion.setIsLoopFadeIn?.(false);
        };

        Object.entries(motionManager.motionGroups ?? {}).forEach(
            ([group, motions]) => {
                if (!Array.isArray(motions)) {
                    return;
                }

                motions.forEach((motion, index) =>
                    patchMotion(group, index, motion),
                );
            },
        );

        const handleMotionLoaded = (
            group: string,
            index: number,
            motion: any,
        ) => {
            patchMotion(group, index, motion);
        };

        motionManager.on?.("motionLoaded", handleMotionLoaded);

        return () => {
            motionManager.off?.("motionLoaded", handleMotionLoaded);
        };
    }

    function loadScriptOnce(src: string) {
        const existingPromise = scriptPromises.get(src);
        if (existingPromise) {
            return existingPromise;
        }

        const promise = new Promise<void>((resolve, reject) => {
            const existingScript = Array.from(document.scripts).find(
                (script) => script.dataset.openduckLive2dSrc === src,
            );

            if (existingScript?.dataset.loaded === "true") {
                resolve();
                return;
            }

            const script = existingScript ?? document.createElement("script");
            script.src = src;
            script.async = true;
            script.crossOrigin = "anonymous";
            script.dataset.openduckLive2dSrc = src;
            script.onload = () => {
                script.dataset.loaded = "true";
                resolve();
            };
            script.onerror = () =>
                reject(new Error(`Failed to load Live2D runtime from ${src}`));

            if (!existingScript) {
                document.head.appendChild(script);
            }
        });

        scriptPromises.set(src, promise);
        return promise;
    }

    async function configureZipLoader(ZipLoader: any) {
        if (zipLoaderConfigured) {
            return;
        }

        const { default: JSZip } = await import("jszip");
        ZipLoader.zipReader = async (data: Blob) => JSZip.loadAsync(data);
        ZipLoader.getFilePaths = async (reader: any) =>
            Object.keys(reader.files).filter((path) => !reader.files[path].dir);
        ZipLoader.readText = async (reader: any, path: string) => {
            const entry = reader.file(path);
            if (!entry) {
                throw new Error(`Missing file in Cubism zip: ${path}`);
            }
            return entry.async("text");
        };
        ZipLoader.getFiles = async (reader: any, paths: string[]) =>
            Promise.all(
                paths.map(async (path) => {
                    const entry = reader.file(path);
                    if (!entry) {
                        throw new Error(`Missing file in Cubism zip: ${path}`);
                    }
                    const blob = await entry.async("blob");
                    return new File([blob], path.split("/").pop() ?? path, {
                        type: blob.type,
                    });
                }),
            );
        ZipLoader.releaseReader = () => {};
        zipLoaderConfigured = true;
    }

    async function loadLive2dLibraries() {
        await loadScriptOnce(CUBISM_CORE_URL);

        const PIXI = await import("pixi.js");
        (window as typeof window & { PIXI?: unknown }).PIXI = PIXI;
        const { Live2DModel, ZipLoader } = await import(
            "pixi-live2d-display/cubism4"
        );
        await configureZipLoader(ZipLoader);
        Live2DModel.registerTicker(PIXI.Ticker);

        return { PIXI, Live2DModel };
    }

    async function resolveLive2dSource(config: CubismModelConfig) {
        if (config.source === "zip" || (config.zipId && !config.url)) {
            const zipBlob = await loadStoredContactCubismModelZip(
                config.zipId ?? "",
            );
            if (!zipBlob) {
                throw new Error("Stored Cubism model zip was not found.");
            }

            return [
                new File(
                    [zipBlob],
                    config.zipName || "cubism-model.model3.zip",
                    {
                        type: zipBlob.type || "application/zip",
                    },
                ),
            ];
        }

        if (!config.url) {
            throw new Error("Cubism model URL is missing.");
        }

        return config.url;
    }

    function layoutModel() {
        if (!containerEl || !model) {
            return;
        }

        const width = containerEl.clientWidth;
        const height = containerEl.clientHeight;
        if (width <= 0 || height <= 0) {
            return;
        }

        const currentScaleX = model.scale?.x || 1;
        const currentScaleY = model.scale?.y || 1;
        const naturalWidth = Math.max(model.width / currentScaleX, 1);
        const naturalHeight = Math.max(model.height / currentScaleY, 1);
        const configuredScale = cubismModel.scale ?? 3.5;
        const zoom = cubismModel.zoom ?? 1.0;
        const scale =
            Math.min(width / naturalWidth, height / naturalHeight) *
            configuredScale *
            zoom;
        const scaledHeight = naturalHeight * scale;

        model.scale.set(scale);
        model.x = width / 2 + (cubismModel.offsetX ?? 0);
        model.y =
            scaledHeight +
            (cubismModel.offsetY ??
                (cubismModel.scale ? 0 : scaledHeight * 0.15));
    }

    function destroyLive2d() {
        resizeObserver?.disconnect();
        resizeObserver = null;
        detachMotionLoopPatch?.();
        detachMotionLoopPatch = null;
        model = null;

        if (app) {
            app.destroy(false, {
                children: true,
                texture: false,
                baseTexture: false,
            });
            app = null;
        }
    }

    async function initializeLive2d(token: number, config: CubismModelConfig) {
        if (!containerEl || !canvasEl) {
            return;
        }

        try {
            loadState = "loading";
            const { PIXI, Live2DModel } = await loadLive2dLibraries();
            if (token !== loadToken || !containerEl || !canvasEl) {
                return;
            }

            const nextApp = new PIXI.Application({
                view: canvasEl,
                autoStart: true,
                backgroundAlpha: 0,
                antialias: true,
                resolution: window.devicePixelRatio || 1,
                resizeTo: containerEl,
            });

            if (token !== loadToken) {
                nextApp.destroy(false, {
                    children: true,
                    texture: false,
                    baseTexture: false,
                });
                return;
            }

            app = nextApp;
            const source = await resolveLive2dSource(config);
            const nextModel = await Live2DModel.from(source, {
                autoInteract: false,
            });
            if (token !== loadToken || !containerEl) {
                nextModel.destroy();
                if (app === nextApp) {
                    nextApp.destroy(false, {
                        children: true,
                        texture: false,
                        baseTexture: false,
                    });
                    app = null;
                }
                return;
            }

            model = nextModel;
            detachMotionLoopPatch = configureLoopingCubismMotions(model);
            model.anchor.set(0.5, 1);
            nextApp.stage.addChild(model);
            layoutModel();

            resizeObserver = new ResizeObserver(layoutModel);
            resizeObserver.observe(containerEl);
            loadState = "ready";
        } catch (err) {
            console.error("Failed to initialize Live2D avatar:", err);
            if (token === loadToken) {
                destroyLive2d();
                loadState = "error";
            }
        }
    }

    const currentExpression = $derived(expression ?? cubismModel?.expression ?? null);

    $effect(() => {
        if (model) {
            model.expression(currentExpression);
        }
    });

    $effect(() => {
        const activeModel = model;
        if (!activeModel) {
            return;
        }

        let renderedMouthOpen = 0;
        let desiredMouthOpen = 0;
        let frameId: number;
        const applyMouthOpen = () => {
            setModelMouthOpen(activeModel, desiredMouthOpen);
        };
        const hookedBeforeModelUpdate =
            typeof activeModel.internalModel?.on === "function" &&
            typeof activeModel.internalModel?.off === "function";

        if (hookedBeforeModelUpdate) {
            activeModel.internalModel.on("beforeModelUpdate", applyMouthOpen);
        }

        const animate = () => {
            const hasExternalLipSync = typeof lipSyncValue === "number";
            const target = !calling
                ? 0
                : hasExternalLipSync
                  ? clamp01(lipSyncValue)
                  : speaking
                    ? ((Math.sin(performance.now() * 0.015) + 1) / 2) * 0.8
                    : 0;
            const smoothing = target > renderedMouthOpen ? 0.58 : 0.32;

            renderedMouthOpen += (target - renderedMouthOpen) * smoothing;
            if (renderedMouthOpen < 0.01 && target === 0) {
                renderedMouthOpen = 0;
            }

            desiredMouthOpen = renderedMouthOpen;
            if (!hookedBeforeModelUpdate) {
                applyMouthOpen();
            }
            frameId = requestAnimationFrame(animate);
        };

        frameId = requestAnimationFrame(animate);

        return () => {
            cancelAnimationFrame(frameId);
            activeModel.internalModel?.off?.(
                "beforeModelUpdate",
                applyMouthOpen,
            );
            setModelMouthOpen(activeModel, 0);
        };
    });

    $effect(() => {
        const hasSource = Boolean(
            cubismModel?.url || cubismModel?.source === "zip" || cubismModel?.zipId,
        );

        if (!containerEl || !canvasEl || !hasSource) {
            destroyLive2d();
            return;
        }

        const token = ++loadToken;
        untrack(() => {
            void initializeLive2d(token, $state.snapshot(cubismModel));
        });

        return () => {
            ++loadToken;
            destroyLive2d();
        };
    });
</script>

<div
    class="live2d-avatar"
    class:calling
    class:speaking
    class:ready={loadState === "ready"}
    bind:this={containerEl}
>
    <canvas bind:this={canvasEl} aria-hidden="true"></canvas>
    {#if loadState !== "ready"}
        <div
            class="avatar live2d-avatar-fallback"
            class:calling
            style={fallbackImageStyle}
        ></div>
    {/if}
</div>
