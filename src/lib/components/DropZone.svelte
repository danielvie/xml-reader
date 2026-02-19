<script lang="ts">
    import { appState } from "$lib/state.svelte";
    import { getCurrentWebview } from "@tauri-apps/api/webview";

    let isDragging = $state(false);

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
    <div class="text-center pointer-events-none">
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
    </div>
</div>
