<script lang="ts">
    import { Button } from "$lib/components/ui/button";
    import * as Card from "$lib/components/ui/card";
    import { invoke } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";

    type InstanceCard = {
        id: string;
        name: string;
        software: string;
        version: string;
        running: boolean;
        playit: boolean;
    };

    let instances: InstanceCard[] = $state([]);

    function openNewInstanceWindow() {
        invoke("open_new_instance_window");
    }

    function handleView(id: string, name: string) {
        invoke("open_instance_view", { id, name });
    }

    function fetchInstances() {
        invoke<InstanceCard[]>("list_instances").then(
            (list) => (instances = list),
        );
    }

    function statusLabel(instance: InstanceCard) {
        if (instance.running && instance.playit) {
            return "Running • Playit";
        }
        if (instance.running) {
            return "Running";
        }
        if (instance.playit) {
            return "Stopped • Playit";
        }
        return "Stopped";
    }

    $effect(() => {
        fetchInstances();

        const unlistenPromise = listen("instances-updated", () => {
            fetchInstances();
        });

        return () => {
            unlistenPromise.then((unlisten) => unlisten());
        };
    });
</script>

<main class="min-h-screen bg-background text-foreground m-0 p-0">
    <aside
        class="font-sans h-10 bg-sidebar flex border-b border-border items-center"
    >
        <Button
            class="bg-primary text-primary-foreground rounded-none px-4 py-2 hover:bg-primary/90 flex items-center gap-2 transition-colors duration-100 cursor-pointer"
            onclick={openNewInstanceWindow}
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                width="24"
                height="24"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="lucide lucide-circle-plus-icon lucide-circle-plus"
                ><circle cx="12" cy="12" r="10" /><path d="M8 12h8" /><path
                    d="M12 8v8"
                /></svg
            >
            New Instance
        </Button>
    </aside>
    <div
        class="m-10 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4"
    >
        {#each instances as i}
            <Card.Root class="relative">
                <Card.Header class="space-y-2">
                    <div class="flex items-center justify-between gap-2">
                        <Card.Title class="font-mono truncate"
                            >{i.name}</Card.Title
                        >
                        {#if i.playit}
                            <span
                                class="text-xs uppercase tracking-wide px-2 py-0.5 rounded-md bg-emerald-600/15 text-emerald-400 border border-emerald-600/40"
                            >
                                Playit
                            </span>
                        {/if}
                    </div>
                    <Card.Description class="flex flex-col gap-1 text-sm">
                        <span>{i.software} v{i.version}</span>
                        <span
                            class={`font-semibold ${
                                i.running
                                    ? "text-emerald-400"
                                    : "text-muted-foreground"
                            }`}>{statusLabel(i)}</span
                        >
                    </Card.Description>
                </Card.Header>
                <Card.Footer class="flex justify-between items-center">
                    <div
                        class="text-xs text-muted-foreground flex items-center gap-1"
                    >
                        <span
                            class={`inline-block h-2 w-2 rounded-full ${
                                i.running
                                    ? "bg-emerald-400"
                                    : "bg-muted-foreground"
                            }`}
                        ></span>
                        {i.running ? "Online" : "Offline"}
                    </div>
                    <Button
                        class="cursor-pointer"
                        onclick={() => handleView(i.id, i.name)}
                    >
                        View
                    </Button>
                </Card.Footer>
            </Card.Root>
        {/each}
    </div>
</main>
