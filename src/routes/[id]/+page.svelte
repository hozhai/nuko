<script lang="ts">
    import { page } from "$app/state";
    import { tick } from "svelte";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import { invoke } from "@tauri-apps/api/core";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { ChartContainer, ChartTooltip } from "$lib/components/ui/chart";
    import { AreaChart } from "layerchart";
    import { scaleTime } from "d3-scale";

    let uuid = page.params.id;

    let logs = $state<string[]>([]);
    let logContainer = $state<HTMLElement | null>(null);
    let isRunning = $state(false);
    let metrics = $state<{ time: Date; cpu: number; memory: number }[]>([]);
    let commandInput = $state("");
    let commandHistory = $state<string[]>([]);
    let historyIndex = $state(-1);
    let historyDraft = $state("");

    function fetchInstanceInfo() {
        invoke<{ running: boolean }>("get_instance_info", { id: uuid })
            .then((info) => {
                isRunning = info.running;
            })
            .catch(console.error);
    }

    $effect(() => {
        fetchInstanceInfo();

        let unlistenInfo: UnlistenFn;
        listen("instances-updated", () => {
            fetchInstanceInfo();
        }).then((fn) => {
            unlistenInfo = fn;
        });

        invoke<string[]>("get_instance_logs", { id: uuid })
            .then(async (initialLogs) => {
                logs = initialLogs;
                await tick();
                if (logContainer) {
                    logContainer.scrollTop = logContainer.scrollHeight;
                }
            })
            .catch(console.error);

        let unlisten: UnlistenFn;

        listen<string>(`instance-log-${uuid}`, async (event) => {
            logs.push(event.payload);

            await tick();
            if (logContainer) {
                logContainer.scrollTop = logContainer.scrollHeight;
            }
        }).then((fn) => {
            unlisten = fn;
        });

        return () => {
            if (unlisten) unlisten();
            if (unlistenInfo) unlistenInfo();
        };
    });

    $effect(() => {
        let interval: ReturnType<typeof setInterval>;
        if (isRunning) {
            interval = setInterval(() => {
                invoke<{
                    time: string;
                    cpu_usage: number;
                    memory_usage: number;
                }>("get_instance_metrics", { id: uuid })
                    .then((m) => {
                        const cutoff = Date.now() - 30000;
                        const newMetrics = [
                            ...metrics,
                            {
                                time: new Date(),
                                cpu: Number(m.cpu_usage.toFixed(2)),
                                memory: Number(
                                    (m.memory_usage / 1024 / 1024).toFixed(2),
                                ),
                            },
                        ].filter((entry) => entry.time.getTime() >= cutoff);
                        metrics = newMetrics;
                    })
                    .catch(console.error);
            }, 1000);
        } else {
            metrics = [];
        }
        return () => {
            if (interval) clearInterval(interval);
        };
    });

    function startServer() {
        logs = [];
        invoke("start_instance", { id: uuid }).catch(console.error);
    }

    function stopServer() {
        invoke("stop_instance", { id: uuid }).catch(console.error);
    }

    function restartServer() {
        logs = [];
        invoke("restart_instance", { id: uuid }).catch(console.error);
    }

    function killServer() {
        invoke("kill_instance", { id: uuid }).catch(console.error);
    }

    async function sendCommand() {
        const trimmed = commandInput.trim();
        if (!trimmed) return;
        logs = [...logs, `> ${trimmed}`];
        await tick();
        if (logContainer) {
            logContainer.scrollTop = logContainer.scrollHeight;
        }
        commandHistory = [...commandHistory, trimmed];
        historyIndex = -1;
        historyDraft = "";
        invoke("send_instance_command", {
            id: uuid,
            command: trimmed,
        }).catch(console.error);
        commandInput = "";
    }
</script>

<main
    class="min-h-screen bg-background text-foreground p-6 flex flex-col gap-4"
>
    <div class="flex justify-between items-center">
        <h1 class="text-2xl font-bold">Instance Console</h1>
        <div class="flex gap-2">
            <Button
                variant={isRunning ? "secondary" : "default"}
                disabled={isRunning}
                onclick={startServer}
                class="cursor-pointer">Start</Button
            >
            <Button
                variant="secondary"
                disabled={!isRunning}
                onclick={stopServer}
                class="cursor-pointer">Stop</Button
            >
            <Button
                variant="secondary"
                disabled={!isRunning}
                onclick={restartServer}
                class="cursor-pointer">Restart</Button
            >
            <Button
                variant="destructive"
                disabled={!isRunning}
                onclick={killServer}>Kill</Button
            >
        </div>
    </div>

    <!-- Terminal Window -->
    <div
        class="flex-1 bg-muted/50 rounded-lg border p-4 overflow-hidden flex flex-col shadow-inner min-h-[500px] max-h-[500px] overflow-y-scroll"
    >
        <div
            bind:this={logContainer}
            class="flex-1 overflow-y-auto font-mono text-sm text-foreground space-y-1 pr-2"
        >
            {#if logs.length === 0}
                <div class="text-muted-foreground italic">
                    Waiting for server logs...
                </div>
            {/if}
            {#each logs as log}
                <div class="break-all hover:bg-foreground/5 px-1 rounded">
                    {log}
                </div>
            {/each}
        </div>
    </div>

    <div class="flex gap-2 items-center">
        <Input
            type="text"
            class="font-mono flex-1"
            placeholder="Type a command and press Enter..."
            bind:value={commandInput}
            disabled={!isRunning}
            onkeydown={(e) => {
                if (e.key === "Enter") {
                    sendCommand();
                    return;
                }
                if (e.key === "ArrowUp") {
                    if (commandHistory.length === 0) return;
                    e.preventDefault();
                    if (historyIndex === -1) {
                        historyDraft = commandInput;
                        historyIndex = commandHistory.length - 1;
                    } else if (historyIndex > 0) {
                        historyIndex -= 1;
                    }
                    commandInput = commandHistory[historyIndex] ?? commandInput;
                    return;
                }
                if (e.key === "ArrowDown") {
                    if (commandHistory.length === 0 || historyIndex === -1)
                        return;
                    e.preventDefault();
                    if (historyIndex < commandHistory.length - 1) {
                        historyIndex += 1;
                        commandInput =
                            commandHistory[historyIndex] ?? commandInput;
                    } else {
                        historyIndex = -1;
                        commandInput = historyDraft;
                    }
                }
            }}
        />
        <Button
            variant="secondary"
            disabled={!isRunning || !commandInput.trim()}
            onclick={sendCommand}
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
                class="lucide lucide-send-horizontal-icon lucide-send-horizontal"
                ><path
                    d="M3.714 3.048a.498.498 0 0 0-.683.627l2.843 7.627a2 2 0 0 1 0 1.396l-2.842 7.627a.498.498 0 0 0 .682.627l18-8.5a.5.5 0 0 0 0-.904z"
                /><path d="M6 12h16" /></svg
            >
        </Button>
    </div>

    <!-- Metrics -->
    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div class="bg-card border rounded-lg p-4 flex flex-col gap-2">
            <h2 class="font-semibold">CPU Usage (%)</h2>
            <div class="h-50 w-full">
                <ChartContainer
                    class="h-50"
                    config={{
                        cpu: { label: "CPU", color: "var(--chart-1)" },
                    }}
                >
                    <AreaChart
                        data={metrics}
                        x="time"
                        y="cpu"
                        xScale={scaleTime()}
                        yDomain={[0, null]}
                        axis={isRunning ? "y" : false}
                        series={[{ key: "cpu", color: "var(--chart-1)" }]}
                    >
                        {#snippet tooltip()}
                            <ChartTooltip />
                        {/snippet}
                    </AreaChart>
                </ChartContainer>
            </div>
        </div>
        <div class="bg-card border rounded-lg p-4 flex flex-col gap-2">
            <h2 class="font-semibold">Memory Usage (MB)</h2>
            <div class="h-50 w-full">
                <ChartContainer
                    class="h-50"
                    config={{
                        memory: {
                            label: "Memory",
                            color: "var(--chart-2)",
                        },
                    }}
                >
                    <AreaChart
                        data={metrics}
                        x="time"
                        y="memory"
                        xScale={scaleTime()}
                        yDomain={[0, null]}
                        axis={isRunning ? "y" : false}
                        series={[{ key: "memory", color: "var(--chart-2)" }]}
                    >
                        {#snippet tooltip()}
                            <ChartTooltip />
                        {/snippet}
                    </AreaChart>
                </ChartContainer>
            </div>
        </div>
    </div>
</main>
