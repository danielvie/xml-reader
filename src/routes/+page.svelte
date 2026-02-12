<script lang="ts">
    import { appState } from "$lib/state.svelte";
    import DropZone from "$lib/components/DropZone.svelte";
    import Viewer from "$lib/components/Viewer.svelte";
</script>

<div class="h-screen w-screen bg-gray-950 text-gray-100 font-sans">
    {#if !appState.currentFile}
        <div
            class="container mx-auto h-full flex items-center justify-center gap-8 p-8"
        >
            <DropZone />

            <!-- Recent Files Panel -->
            {#if appState.recentFiles.length > 0}
                <div
                    class="w-72 max-h-120 flex flex-col bg-gray-900/60 border border-gray-700 rounded-2xl p-5 overflow-hidden"
                >
                    <h3
                        class="text-xs uppercase tracking-wider text-gray-500 font-semibold mb-3 shrink-0"
                    >
                        Recent Files
                    </h3>
                    <div class="flex-1 overflow-y-auto space-y-1.5 pr-1">
                        {#each appState.recentFiles as file (file.path)}
                            <div
                                class="w-full group flex items-center gap-2 px-3 py-2 rounded-lg text-left hover:bg-gray-800 transition-colors cursor-pointer"
                                role="button"
                                tabindex="0"
                                onclick={() => appState.openFile(file.path)}
                                onkeydown={(e) => {
                                    if (e.key === "Enter" || e.key === " ")
                                        appState.openFile(file.path);
                                }}
                                title={file.path}
                            >
                                <span
                                    class="text-gray-500 group-hover:text-blue-400 transition-colors shrink-0"
                                    >📄</span
                                >
                                <div class="min-w-0 flex-1">
                                    <p
                                        class="text-sm text-gray-300 group-hover:text-white truncate transition-colors font-medium"
                                    >
                                        {file.name}
                                    </p>
                                    <p
                                        class="text-[10px] text-gray-600 truncate"
                                    >
                                        {file.path}
                                    </p>
                                </div>
                                <button
                                    class="opacity-0 group-hover:opacity-100 shrink-0 w-5 h-5 flex items-center justify-center rounded text-gray-600 hover:text-red-400 hover:bg-gray-700 transition-all text-xs"
                                    onclick={(e) => {
                                        e.stopPropagation();
                                        appState.removeFromRecentFiles(
                                            file.path,
                                        );
                                    }}
                                    title="Remove from recent">✕</button
                                >
                            </div>
                        {/each}
                    </div>
                </div>
            {/if}
        </div>
    {:else}
        <Viewer />
    {/if}
</div>
