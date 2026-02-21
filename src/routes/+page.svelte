<script lang="ts">
    import { Button } from "$lib/components/ui/button";
    import * as Card from "$lib/components/ui/card";
    import { invoke } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";

    let instances: Array<{
        name: string;
        software: string;
        version: string;
        running: boolean;
    }> = $state([]);

    function openNewInstanceWindow() {
        invoke("open_new_instance_window");
    }

    function fetchInstances() {
        invoke<
            Array<{
                name: string;
                software: string;
                version: string;
                running: boolean;
            }>
        >("list_instances").then((list) => (instances = list));
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
    <nav
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
    </nav>
    <div
        class="m-10 grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4"
    >
        {#each instances as i}
            <Card.Root>
                <Card.Header>
                    <Card.Title>{i.name}</Card.Title>
                    <Card.Description>
                        {i.software} v{i.version} - {i.running
                            ? "Running"
                            : "Stopped"}
                    </Card.Description>
                </Card.Header>
                <Card.Footer>
                    <Button>View</Button>
                </Card.Footer>
            </Card.Root>
        {/each}
    </div>
</main>
