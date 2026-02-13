<script lang="ts">
    import { appState } from "$lib/state.svelte";
    import IconElementFirst from "$lib/components/icons/IconElementFirst.svelte";
    import IconElementSecond from "$lib/components/icons/IconElementSecond.svelte";
    import IconHome from "$lib/components/icons/IconHome.svelte";

    let searchQuery = $state("");
    let copiedFlash = $state(false);

    function handleSearch(next: boolean) {
        if (!searchQuery) return;
        appState.performSearch(searchQuery, next);
    }

    async function copyXpath() {
        if (appState.currentXpath) {
            try {
                await navigator.clipboard.writeText(appState.currentXpath);
                copiedFlash = true;
                setTimeout(() => (copiedFlash = false), 1500);
            } catch {
                prompt("Copy XPath:", appState.currentXpath);
            }
        }
    }
</script>

<header
    class="h-14 bg-gray-900/90 backdrop-blur-md border-b border-gray-800 flex items-center px-4 gap-3 z-10 sticky top-0 shrink-0"
>
    <!-- File name + nav buttons -->
    <div
        class="flex items-center gap-1.5 min-w-0 shrink-0"
        title={appState.currentFile}
    >
        <button
            onclick={() => appState.closeFile()}
            class="w-7 h-7 flex items-center justify-center hover:bg-gray-800 rounded-sm text-gray-400 hover:text-white transition-colors text-sm"
            title="Back to Home"
        >
            <IconHome />
        </button>
        <span class="text-gray-700">|</span>
        <span class="font-mono text-xs text-gray-400 truncate max-w-30">
            {appState.fileName}
        </span>
        <span class="text-gray-700">|</span>
        <button
            onclick={() => appState.goToStart()}
            class="w-7 h-8 flex items-center justify-center hover:bg-gray-800 rounded-sm text-gray-400 hover:text-white transition-colors"
            title="Go to first element"
        >
            <IconElementFirst />
        </button>
        <button
            onclick={() => appState.goToEnd()}
            class="w-7 h-8 flex items-center justify-center hover:bg-gray-800 rounded-sm text-gray-400 hover:text-white transition-colors"
            title="Go to last element"
        >
            <IconElementSecond />
        </button>
    </div>

    <!-- Search -->
    <div
        class="flex items-center gap-1.5 border border-gray-700 rounded-sm px-3 py-1 bg-gray-800/50 focus-within:border-blue-500 focus-within:ring-1 focus-within:ring-blue-500/30 transition-colors w-56 shrink-0"
    >
        <svg
            class="text-gray-500 shrink-0"
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <circle cx="11" cy="11" r="7" />
            <line x1="16.5" y1="16.5" x2="21" y2="21" />
        </svg>
        <div class="relative flex-1">
            <input
                type="text"
                bind:value={searchQuery}
                onkeydown={(e) => e.key === "Enter" && handleSearch(true)}
                placeholder="Find tag..."
                class="w-full bg-transparent py-1 px-3 font-mono border-none outline-none text-xs text-gray-200 placeholder-gray-600"
            />
            {#if appState.isSearching}
                <span
                    class="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] text-blue-400 animate-pulse bg-gray-900/80 px-1"
                >
                    searching... {appState.searchProgress}%
                </span>
            {/if}
        </div>
    </div>

    <!-- Cancel / Prev / Next -->
    <div class="flex gap-0.5 shrink-0">
        {#if appState.isSearching}
            <button
                onclick={() => appState.cancelSearch()}
                class="w-7 h-7 flex items-center justify-center bg-red-900/60 hover:bg-red-800 rounded-sm text-red-300 hover:text-white transition-colors"
                title="Cancel search"
            >
                <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                >
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
            </button>
        {/if}
        <button
            onclick={() => handleSearch(false)}
            class="w-7 h-7 flex items-center justify-center hover:bg-gray-800 rounded-sm text-gray-400 hover:text-white transition-colors"
            title="Previous match"
        >
            <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <polyline points="15 18 9 12 15 6" />
            </svg>
        </button>
        <button
            onclick={() => handleSearch(true)}
            class="w-7 h-7 flex items-center justify-center hover:bg-gray-800 rounded-sm text-gray-400 hover:text-white transition-colors"
            title="Next match"
        >
            <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
            >
                <polyline points="9 6 15 12 9 18" />
            </svg>
        </button>
    </div>

    <!-- XPath -->
    <button
        type="button"
        class="flex-1 min-w-0 group relative cursor-pointer text-left"
        onclick={copyXpath}
        title="Click to copy"
    >
        <div
            class="font-mono text-sm bg-gray-800/60 border border-gray-700 py-1 px-3 rounded truncate text-gray-300 hover:border-blue-500 transition-colors {copiedFlash
                ? 'border-green-500 text-green-300'
                : ''} {appState.isLoadingElement
                ? 'ring-1 ring-blue-500/50'
                : ''} {appState.searchNotFound
                ? 'border-red-500 ring-1 ring-red-500/30'
                : ''}"
        >
            {#if appState.searchNotFound}
                <span class="flex items-center gap-2 text-red-400">
                    <svg
                        width="14"
                        height="14"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <circle cx="12" cy="12" r="10" />
                        <line x1="15" y1="9" x2="9" y2="15" />
                        <line x1="9" y1="9" x2="15" y2="15" />
                    </svg>
                    Not found
                </span>
            {:else if appState.isLoadingElement}
                <span
                    class="flex items-center gap-2 text-blue-400 animate-pulse"
                >
                    <svg
                        class="animate-spin -ml-1 mr-1 h-3 w-3"
                        xmlns="http://www.w3.org/2000/svg"
                        fill="none"
                        viewBox="0 0 24 24"
                    >
                        <circle
                            class="opacity-25"
                            cx="12"
                            cy="12"
                            r="10"
                            stroke="currentColor"
                            stroke-width="4"
                        ></circle>
                        <path
                            class="opacity-75"
                            fill="currentColor"
                            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                        ></path>
                    </svg>
                    Locating element...
                </span>
            {:else}
                <span
                    class="opacity-40 text-[10px] mr-1.5 uppercase tracking-wider"
                    >xpath</span
                >
                {appState.currentXpath || "/"}
            {/if}
        </div>

        <!-- Hover tooltip -->
        <div
            class="absolute top-full left-0 mt-1 w-full max-w-xl bg-gray-800 border border-gray-600 rounded-lg p-3 shadow-2xl opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50"
        >
            <div
                class="font-mono text-xs text-gray-200 break-all leading-relaxed"
            >
                {appState.currentXpath || "No selection"}
            </div>
            <div class="text-right text-gray-500 mt-2 text-[10px]">
                {copiedFlash ? "✅ Copied!" : "Click to copy"}
            </div>
        </div>
    </button>
</header>
