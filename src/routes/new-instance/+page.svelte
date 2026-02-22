<script lang="ts">
    import { invoke, convertFileSrc } from "@tauri-apps/api/core";
    import { open } from "@tauri-apps/plugin-dialog";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Label } from "$lib/components/ui/label";
    import * as Select from "$lib/components/ui/select";
    import { Spinner } from "$lib/components/ui/spinner";

    // Server software options
    const SERVER_SOFTWARE = [
        { value: "vanilla", label: "Vanilla" },
        { value: "papermc", label: "PaperMC" },
        { value: "purpur", label: "Purpur" },
        { value: "fabric", label: "Fabric" },
        { value: "forge", label: "Forge" },
        { value: "neoforge", label: "NeoForge" },
        { value: "custom", label: "Custom JAR" },
    ];

    // Form state
    let name = $state("");
    let software = $state("vanilla");
    let mcVersion = $state("");
    let loaderVersion = $state("");
    let customJarPath = $state<string | null>(null);
    let iconPath = $state<string | null>(null);
    let iconUrl = $state<string | null>(null);

    // Loading states
    let mcVersions = $state<string[]>([]);
    let loaderVersions = $state<string[]>([]);
    let mcLoading = $state(false);
    let loaderLoading = $state(false);
    let creationLoading = $state(false);

    // Error states
    let mcError = $state<string | null>(null);
    let loaderError = $state<string | null>(null);
    let creationError = $state<string | null>(null);

    // Derived state
    let showLoader = $derived(
        software === "fabric" ||
            software === "forge" ||
            software === "neoforge",
    );
    let loaderLabel = $derived(
        software === "fabric"
            ? "Fabric Loader Version"
            : software === "forge"
              ? "Forge Version"
              : software === "neoforge"
                ? "NeoForge Version"
                : "Loader Version",
    );

    let isFormValid = $derived(
        name.trim() !== "" &&
            (software === "custom"
                ? customJarPath !== null
                : mcVersion !== "") &&
            (!showLoader || loaderVersion !== ""),
    );

    // Get the Tauri command name for MC versions
    function getMcVersionsCommand(sw: string) {
        switch (sw) {
            case "vanilla":
                return "get_vanilla_versions";
            case "papermc":
                return "get_paper_versions";
            case "purpur":
                return "get_purpur_versions";
            case "fabric":
                return "get_fabric_game_versions";
            case "forge":
                return "get_forge_mc_versions";
            case "neoforge":
                return "get_neoforge_mc_versions";
            default:
                return "get_vanilla_versions";
        }
    }

    // Get the Tauri command name for loader versions
    function getLoaderVersionsCommand(sw: string) {
        switch (sw) {
            case "fabric":
                return "get_fabric_loader_versions";
            case "forge":
                return "get_forge_versions";
            case "neoforge":
                return "get_neoforge_versions";
            default:
                return null;
        }
    }

    // Fetch MC versions when software changes
    async function fetchMcVersions(sw: string) {
        if (sw === "custom") return;

        mcVersion = "";
        loaderVersion = "";
        mcVersions = [];
        loaderVersions = [];
        mcError = null;
        loaderError = null;
        mcLoading = true;

        try {
            const cmd = getMcVersionsCommand(sw);
            const versions = await invoke<string[]>(cmd);
            mcVersions = versions;
        } catch (e) {
            mcError = `Failed to fetch versions: ${e}`;
        } finally {
            mcLoading = false;
        }
    }

    // Fetch loader versions when MC version changes
    async function fetchLoaderVersions(sw: string, mcv: string) {
        loaderVersion = "";
        loaderVersions = [];
        loaderError = null;

        const cmd = getLoaderVersionsCommand(sw);
        if (!cmd || !mcv) {
            return;
        }

        loaderLoading = true;

        try {
            const versions = await invoke<string[]>(cmd, { mcVersion: mcv });
            loaderVersions = versions;
        } catch (e) {
            loaderError = `Failed to fetch loader versions: ${e}`;
        } finally {
            loaderLoading = false;
        }
    }

    // React to software changes
    $effect(() => {
        fetchMcVersions(software);
    });

    // React to MC version changes (for loader versions)
    $effect(() => {
        if (showLoader && mcVersion) {
            fetchLoaderVersions(software, mcVersion);
        }
    });

    // Event handlers
    function handleCancel() {
        invoke("close_current_window");
    }

    async function handleIconSelect() {
        try {
            const selected = await open({
                multiple: false,
                filters: [
                    {
                        name: "Image",
                        extensions: ["png"],
                    },
                ],
            });

            if (selected && typeof selected === "string") {
                const url = convertFileSrc(selected);
                const img = new Image();
                img.onload = () => {
                    if (img.width === 64 && img.height === 64) {
                        iconPath = selected;
                        iconUrl = url;
                        creationError = null;
                    } else {
                        creationError = `Server icon must be exactly 64x64 pixels (selected image is ${img.width}x${img.height}).`;
                    }
                };
                img.onerror = () => {
                    creationError = "Failed to load the selected image.";
                };
                img.src = url;
            }
        } catch (e) {
            creationError = `Failed to select icon: ${e}`;
        }
    }

    async function handleCustomJarSelect() {
        try {
            const selected = await open({
                multiple: false,
                filters: [
                    {
                        name: "Java Archive",
                        extensions: ["jar"],
                    },
                ],
            });

            if (selected && typeof selected === "string") {
                customJarPath = selected;
                creationError = null;
            }
        } catch (e) {
            creationError = `Failed to select custom JAR: ${e}`;
        }
    }

    async function handleCreate() {
        if (!isFormValid) return;
        try {
            creationLoading = true;
            await invoke("create_instance", {
                name: name,
                software: software,
                version: software === "custom" ? "custom" : mcVersion,
                loader: loaderVersion !== "" ? loaderVersion : null,
                iconPath: iconPath,
                customJarPath: customJarPath,
            });

            creationLoading = false;

            await invoke("close_current_window");
        } catch (e) {
            creationError = `Failed to create instance: ${e}`;
            creationLoading = false;
        }
    }
</script>

<main class="min-h-screen bg-background text-foreground m-0 p-0">
    <div class="bg-muted text-foreground p-4 border-b">
        <h1 class="text-2xl font-bold">New Instance</h1>
    </div>

    <div class="flex m-4 space-x-4 items-center p-2 bg-muted rounded-md">
        <!-- Server icon placeholder -->
        <button
            class="relative w-16 h-16 rounded-md overflow-hidden bg-background border border-border flex items-center justify-center hover:opacity-80 transition-opacity cursor-pointer shrink-0"
            onclick={handleIconSelect}
            title="Select server icon (64x64 PNG)"
        >
            {#if iconUrl}
                <img
                    src={iconUrl}
                    alt="Server Icon"
                    class="w-full h-full object-cover"
                />
            {:else}
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="2rem"
                    height="2rem"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    class="text-muted-foreground"
                >
                    <rect x="2" y="2" width="20" height="8" rx="2" ry="2"
                    ></rect>
                    <rect x="2" y="14" width="20" height="8" rx="2" ry="2"
                    ></rect>
                    <line x1="6" y1="6" x2="6.01" y2="6"></line>
                    <line x1="6" y1="18" x2="6.01" y2="18"></line>
                </svg>
            {/if}
        </button>
        <div class="flex-1">
            <Input
                type="text"
                class="font-mono"
                placeholder="Server Name..."
                bind:value={name}
            />
        </div>
    </div>

    <div class="m-4 space-y-4">
        <!-- Server Software Selector -->
        <div>
            <Label for="software" class="mb-2 block">Server Software</Label>
            <Select.Root type="single" bind:value={software}>
                <Select.Trigger id="software" class="w-full">
                    {SERVER_SOFTWARE.find((s) => s.value === software)?.label ??
                        "Select software..."}
                </Select.Trigger>
                <Select.Content>
                    {#each SERVER_SOFTWARE as sw}
                        <Select.Item value={sw.value} label={sw.label}
                            >{sw.label}</Select.Item
                        >
                    {/each}
                </Select.Content>
            </Select.Root>
        </div>

        <!-- Minecraft Version Selector -->
        {#if software !== "custom"}
            <div>
                <Label for="mcVersion" class="mb-2 block">
                    Minecraft Version
                    {#if mcLoading}
                        <span class="ml-2 text-muted-foreground text-xs"
                            >(loading...)</span
                        >
                    {/if}
                </Label>
                <Select.Root
                    type="single"
                    bind:value={mcVersion}
                    disabled={mcLoading}
                >
                    <Select.Trigger id="mcVersion" class="w-full">
                        {mcVersion
                            ? mcVersion
                            : mcLoading
                              ? "Loading versions..."
                              : "Select a version..."}
                    </Select.Trigger>
                    <Select.Content>
                        {#each mcVersions as v}
                            <Select.Item value={v} label={v}>{v}</Select.Item>
                        {/each}
                    </Select.Content>
                </Select.Root>
                {#if mcError}
                    <p class="text-destructive text-sm mt-1">{mcError}</p>
                {/if}
            </div>
        {:else}
            <div>
                <Label class="mb-2 block">Bring your own Server JAR</Label>
                <div class="flex gap-2">
                    <Input
                        type="text"
                        readonly
                        value={customJarPath || "No JAR selected..."}
                        class="flex-1"
                    />
                    <Button variant="secondary" onclick={handleCustomJarSelect}
                        >Browse</Button
                    >
                </div>
            </div>
        {/if}

        <!-- Loader Version Selector (conditional) -->
        {#if showLoader}
            <div>
                <Label for="loaderVersion" class="mb-2 block">
                    {loaderLabel}
                    {#if loaderLoading}
                        <span class="ml-2 text-muted-foreground text-xs"
                            >(loading...)</span
                        >
                    {/if}
                </Label>
                <Select.Root
                    type="single"
                    bind:value={loaderVersion}
                    disabled={loaderLoading || mcVersion === ""}
                >
                    <Select.Trigger id="loaderVersion" class="w-full">
                        {loaderVersion
                            ? loaderVersion
                            : mcVersion === ""
                              ? "Select a Minecraft version first..."
                              : loaderLoading
                                ? "Loading loader versions..."
                                : "Select a loader version..."}
                    </Select.Trigger>
                    <Select.Content>
                        {#each loaderVersions as v}
                            <Select.Item value={v} label={v}>{v}</Select.Item>
                        {/each}
                    </Select.Content>
                </Select.Root>
                {#if loaderError}
                    <p class="text-destructive text-sm mt-1">{loaderError}</p>
                {/if}
            </div>
        {/if}
    </div>

    <!-- Creation error message -->
    {#if creationError}
        <div class="absolute bottom-0 m-5 left-0 text-destructive">
            <p>{creationError}</p>
        </div>
    {/if}

    <!-- Buttons at bottom-right -->
    <div class="absolute bottom-0 right-0 m-4 flex space-x-3">
        {#if !creationLoading}
            <Button variant="secondary" onclick={handleCancel}>Cancel</Button>
        {/if}
        <Button
            class="bg-emerald-600 text-white hover:bg-emerald-500"
            onclick={handleCreate}
            disabled={!isFormValid || creationLoading}
        >
            {#if creationLoading}
                <Spinner />
            {:else}
                Create
            {/if}
        </Button>
    </div>
</main>
