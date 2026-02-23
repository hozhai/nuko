<script>
    import "../app.css";
    import { onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { listen } from "@tauri-apps/api/event";

    onMount(() => {
        let unlisten: any;

        async function setup() {
            try {
                const config = await invoke("get_config");
                localStorage.setItem("theme", config.theme);
                if (config.theme === "dark") {
                    document.documentElement.classList.add("dark");
                } else {
                    document.documentElement.classList.remove("dark");
                }
            } catch (e) {
                console.error("Failed to load config:", e);
            }

            unlisten = await listen("theme-changed", (event) => {
                const theme = event.payload;
                localStorage.setItem("theme", theme);
                if (theme === "dark") {
                    document.documentElement.classList.add("dark");
                } else {
                    document.documentElement.classList.remove("dark");
                }
            });
        }

        setup();

        return () => {
            if (unlisten) {
                unlisten();
            }
        };
    });
</script>

<slot />
