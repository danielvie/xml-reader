<script lang="ts">
    import { appState } from "$lib/state.svelte";
    import Header from "$lib/components/Header.svelte";

    let elementCopied = $state(false);

    async function copyElement() {
        if (appState.contentActive) {
            try {
                await navigator.clipboard.writeText(appState.contentActive);
                elementCopied = true;
                setTimeout(() => (elementCopied = false), 1500);
            } catch {
                prompt("Copy element:", appState.contentActive);
            }
        }
    }

    // --- Pretty Print XML (for active element only) ---
    function prettifyXml(xml: string): string {
        if (!xml) return "";
        let s = xml.replace(/>\s+</g, "><").trim();

        let formatted = "";
        let indent = 0;
        const pad = "  ";

        const tokens = s.split(/(<[^>]+>)/).filter(Boolean);

        for (const token of tokens) {
            if (token.startsWith("</")) {
                indent = Math.max(0, indent - 1);
                formatted += pad.repeat(indent) + token + "\n";
            } else if (token.startsWith("<") && token.endsWith("/>")) {
                formatted += pad.repeat(indent) + token + "\n";
            } else if (token.startsWith("<")) {
                formatted += pad.repeat(indent) + token + "\n";
                indent++;
            } else {
                const trimmed = token.trim();
                if (trimmed) {
                    formatted += pad.repeat(indent) + trimmed + "\n";
                }
            }
        }

        return formatted.trimEnd();
    }

    // --- Syntax Highlighting ---
    function highlightXml(xml: string): string {
        if (!xml) return "";

        let s = xml
            .replace(/&/g, "&amp;")
            .replace(/</g, "&lt;")
            .replace(/>/g, "&gt;");

        const O = (c: string) => `\x01${c}\x02`;
        const C = "\x03";

        s = s.replace(/(&lt;!--[\s\S]*?--&gt;)/g, `${O("xml-comment")}$1${C}`);
        s = s.replace(/(&lt;\/?[a-zA-Z0-9_\-.:]+)/g, `${O("xml-tag")}$1${C}`);
        s = s.replace(
            /(\s)([a-zA-Z0-9_\-.:]+)(=)/g,
            `$1${O("xml-attr")}$2${C}$3`,
        );
        s = s.replace(/(".*?")/g, `${O("xml-val")}$1${C}`);
        s = s.replace(/(\/?\&gt;)/g, `${O("xml-tag")}$1${C}`);

        s = s.replace(/\x01([^\x02]+)\x02/g, '<span class="$1">');
        s = s.replace(/\x03/g, "</span>");

        return s;
    }

    let hasActiveContent = $derived(appState.contentActive.length > 0);

    let highlightedBefore = $derived(highlightXml(appState.contentBefore));
    let highlightedActive = $derived(
        highlightXml(prettifyXml(appState.contentActive)),
    );
    let highlightedAfter = $derived(highlightXml(appState.contentAfter));
    let highlightedFull = $derived(highlightXml(appState.contentWindow));

    $effect(() => {
        // Access viewOffset to make this effect re-run on every new search result
        const _offset = appState.viewOffset;
        if (hasActiveContent) {
            setTimeout(() => {
                const el = document.getElementById("active-section");
                if (el) {
                    el.scrollIntoView({ behavior: "smooth", block: "center" });
                }
            }, 80);
        }
    });
</script>

<div class="flex flex-col h-screen bg-gray-950 text-gray-100">
    <Header />

    <!-- ═══════════════ CONTENT ═══════════════ -->
    <div class="flex-1 overflow-auto bg-gray-950 font-mono text-sm relative">
        {#if hasActiveContent}
            <!-- THREE-PANEL VIEW -->

            <!-- Previous Neighbors -->
            {#if appState.contentBefore}
                <div class="px-6 pt-6 pb-2 opacity-60">
                    <div class="whitespace-pre-wrap break-all leading-relaxed">
                        {@html highlightedBefore}
                    </div>
                </div>
            {/if}

            <!-- Active Element -->
            <div
                id="active-section"
                class="mx-4 px-4 py-3 bg-amber-950/40 border-l-4 border-amber-400 rounded-r-lg relative group/active"
            >
                <!-- Parent XPath breadcrumb -->
                {#if appState.parentXpath}
                    <button
                        onclick={() => appState.navigateToParent()}
                        class="mb-2 inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-gray-800/90 border border-gray-600 text-[11px] font-mono text-blue-400 hover:text-blue-300 hover:border-blue-500 hover:bg-gray-700/90 transition-colors cursor-pointer"
                        title="Navigate to parent element"
                    >
                        <svg
                            width="12"
                            height="12"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.5"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        >
                            <polyline points="18 15 12 9 6 15" />
                        </svg>
                        {appState.parentXpath}
                    </button>
                {/if}
                <!-- Copy button -->
                <button
                    onclick={copyElement}
                    class="absolute top-2 right-2 px-2 py-1 text-[10px] rounded bg-gray-800/80 text-gray-400 hover:text-white hover:bg-gray-700 transition-colors opacity-0 group-hover/active:opacity-100 border border-gray-700"
                    title="Copy element XML"
                >
                    {elementCopied ? "✅ Copied" : "📋 Copy"}
                </button>
                <div class="whitespace-pre-wrap break-all leading-relaxed">
                    {@html highlightedActive}
                </div>
            </div>

            <!-- Next Neighbors -->
            {#if appState.contentAfter}
                <div class="px-6 pt-2 pb-6 opacity-60">
                    <div class="whitespace-pre-wrap break-all leading-relaxed">
                        {@html highlightedAfter}
                    </div>
                </div>
            {/if}
        {:else}
            <!-- SINGLE CHUNK VIEW (before any search) -->
            <div class="p-6">
                <div class="whitespace-pre-wrap break-all leading-relaxed">
                    {@html highlightedFull}
                </div>
            </div>
        {/if}
    </div>
</div>

<style>
    :global(.xml-tag) {
        color: #60a5fa;
        font-weight: 600;
    }
    :global(.xml-attr) {
        color: #c4b5fd;
    }
    :global(.xml-val) {
        color: #86efac;
    }
    :global(.xml-comment) {
        color: #6b7280;
        font-style: italic;
    }
</style>
