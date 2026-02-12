<script lang="ts">
    import { appState } from "$lib/state.svelte";
    import { getCurrentWebview } from "@tauri-apps/api/webview";

    let isDragging = $state(false);
    let isGenerating = $state(false);

    // Generation options
    let sizeMb = $state(50);
    let depth = $state(3);

    // --- Improved Drag & Drop Handling ---
    function onDragEnter(e: DragEvent) {
        e.preventDefault();
        e.stopPropagation();
        isDragging = true;
    }

    function onDragLeave(e: DragEvent) {
        e.preventDefault();
        e.stopPropagation();
        isDragging = false;
    }

    function onDragOver(e: DragEvent) {
        e.preventDefault();
        e.stopPropagation();
        isDragging = true;
    }

    async function handleDrop(e: DragEvent) {
        e.preventDefault();
        e.stopPropagation();
        isDragging = false;

        const files = e.dataTransfer?.files;
        if (files && files.length > 0) {
            // Check if it's a file path or a File object
            // For Tauri native drops, we use the $effect listener below.
        }
    }

    // Tauri native file drop listener
    $effect(() => {
        const unlisten = getCurrentWebview().onDragDropEvent((event) => {
            if (event.payload.type === "enter") {
                isDragging = true;
            } else if (
                event.payload.type === "leave" ||
                event.payload.type === "drop"
            ) {
                isDragging = false;
            }

            if (event.payload.type === "drop") {
                const paths = event.payload.paths;
                if (paths && paths.length > 0) {
                    appState.openFile(paths[0]);
                }
            }
        });
        return () => {
            unlisten.then((f) => f());
        };
    });

    async function generate() {
        isGenerating = true;
        try {
            await appState.generateSampleFile(sizeMb, depth);
        } finally {
            isGenerating = false;
        }
    }
</script>

<div
    class="flex flex-col items-center justify-center p-10 border-2 border-dashed rounded-2xl transition-all duration-300 w-full max-w-lg {isDragging
        ? 'border-blue-400 bg-blue-500/10 scale-[1.02] shadow-xl shadow-blue-500/10'
        : 'border-gray-700 bg-gray-900/60'}"
    role="region"
    aria-label="File drop zone"
    ondragenter={onDragEnter}
    ondragleave={onDragLeave}
    ondragover={onDragOver}
    ondrop={handleDrop}
>
    <!-- Drop area -->
    <div class="text-center mb-6 pointer-events-none">
        <div
            class="text-4xl mb-3 transition-transform duration-300 {isDragging
                ? 'scale-125'
                : ''}"
        >
            {isDragging ? "📂" : "📄"}
        </div>
        <p
            class="text-lg font-medium {isDragging
                ? 'text-blue-300'
                : 'text-gray-300'}"
        >
            {isDragging ? "Drop to Open" : "Drag & Drop XML file here"}
        </p>
        <p class="text-xs text-gray-500 mt-1">or generate a sample below</p>
    </div>

    <!-- Divider -->
    <div class="w-full border-t border-gray-800 my-4"></div>

    <!-- Generation Options -->
    <div class="w-full space-y-3">
        <h3
            class="text-xs uppercase tracking-wider text-gray-500 font-semibold"
        >
            Generate Sample XML
        </h3>

        <div class="flex gap-3">
            <!-- Size -->
            <div class="flex-1">
                <label
                    for="gen-size"
                    class="block text-[10px] text-gray-500 mb-1 uppercase tracking-wider"
                >
                    Size (MB)
                </label>
                <input
                    id="gen-size"
                    type="number"
                    bind:value={sizeMb}
                    min="1"
                    max="2048"
                    class="w-full bg-gray-800 border border-gray-700 rounded px-3 py-1.5 text-sm text-gray-200 focus:border-blue-500 focus:outline-none transition-colors"
                />
            </div>

            <!-- Depth -->
            <div class="flex-1">
                <label
                    for="gen-depth"
                    class="block text-[10px] text-gray-500 mb-1 uppercase tracking-wider"
                >
                    Depth
                </label>
                <input
                    id="gen-depth"
                    type="number"
                    bind:value={depth}
                    min="1"
                    max="10"
                    class="w-full bg-gray-800 border border-gray-700 rounded px-3 py-1.5 text-sm text-gray-200 focus:border-blue-500 focus:outline-none transition-colors"
                />
            </div>
        </div>

        <!-- Presets -->
        <div class="flex gap-2">
            <button
                class="text-[10px] px-2 py-1 rounded bg-gray-800 text-gray-400 hover:text-white hover:bg-gray-700 transition-colors border border-gray-700"
                onclick={() => {
                    sizeMb = 10;
                    depth = 2;
                }}>Small (10MB)</button
            >
            <button
                class="text-[10px] px-2 py-1 rounded bg-gray-800 text-gray-400 hover:text-white hover:bg-gray-700 transition-colors border border-gray-700"
                onclick={() => {
                    sizeMb = 100;
                    depth = 3;
                }}>Medium (100MB)</button
            >
            <button
                class="text-[10px] px-2 py-1 rounded bg-gray-800 text-gray-400 hover:text-white hover:bg-gray-700 transition-colors border border-gray-700"
                onclick={() => {
                    sizeMb = 500;
                    depth = 4;
                }}>Large (500MB)</button
            >
            <button
                class="text-[10px] px-2 py-1 rounded bg-gray-800 text-gray-400 hover:text-white hover:bg-gray-700 transition-colors border border-gray-700"
                onclick={() => {
                    sizeMb = 2048;
                    depth = 5;
                }}>Max (2GB)</button
            >
        </div>

        <!-- Generate button -->
        <button
            class="w-full px-6 py-2.5 bg-blue-600 text-white rounded-lg hover:bg-blue-500 transition-colors font-medium text-sm disabled:opacity-50 disabled:cursor-not-allowed"
            onclick={generate}
            disabled={isGenerating}
        >
            {#if isGenerating}
                <span class="animate-pulse">Generating {sizeMb}MB...</span>
            {:else}
                Generate & Open ({sizeMb}MB, depth {depth})
            {/if}
        </button>
    </div>
</div>
