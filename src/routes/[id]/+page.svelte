<script lang="ts">
    import { page } from "$app/state";
    import { tick } from "svelte";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import { invoke } from "@tauri-apps/api/core";
    import { Button } from "$lib/components/ui/button";
    import { ChartContainer, ChartTooltip } from "$lib/components/ui/chart";
    import { AreaChart } from "layerchart";
    import { scaleTime } from "d3-scale";

    let uuid = page.params.id;

    let logs = $state<string[]>([]);
    let logContainer = $state<HTMLElement | null>(null);
    let isRunning = $state(false);
    let metrics = $state<{ time: Date; cpu: number; memory: number }[]>([]);

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
                        const newMetrics = [
                            ...metrics,
                            {
                                time: new Date(),
                                cpu: Number(m.cpu_usage.toFixed(2)),
                                memory: Number(
                                    (m.memory_usage / 1024 / 1024).toFixed(2),
                                ),
                            },
                        ];
                        if (newMetrics.length > 60) {
                            newMetrics.shift();
                        }
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
</script>

<main
    class="min-h-screen bg-background text-foreground p-6 flex flex-col gap-4"
>
    <div class="flex justify-between items-center">
        <h1 class="text-2xl font-bold">Instance Console</h1>
        <div class="flex gap-2">
            <Button
                variant="secondary"
                disabled={isRunning}
                onclick={startServer}>Start</Button
            >
            <Button
                variant="secondary"
                disabled={!isRunning}
                onclick={stopServer}>Stop</Button
            >
            <Button
                variant="secondary"
                disabled={!isRunning}
                onclick={restartServer}>Restart</Button
            >
            <Button
                variant="destructive"
                disabled={!isRunning}
                onclick={killServer}>Kill</Button
            >
        </div>
    </div>

    {#if isRunning || metrics.length > 0}
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div class="bg-card border rounded-lg p-4 flex flex-col gap-2">
                <h2 class="font-semibold">CPU Usage (%)</h2>
                <div class="h-[200px] w-full">
                    <ChartContainer
                        config={{
                            cpu: { label: "CPU", color: "hsl(var(--chart-1))" },
                        }}
                    >
                        <AreaChart
                            data={metrics}
                            x="time"
                            y="cpu"
                            xScale={scaleTime()}
                            yDomain={[0, null]}
                            series={[{ key: "cpu", color: "var(--color-cpu)" }]}
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
                <div class="h-[200px] w-full">
                    <ChartContainer
                        config={{
                            memory: {
                                label: "Memory",
                                color: "hsl(var(--chart-2))",
                            },
                        }}
                    >
                        <AreaChart
                            data={metrics}
                            x="time"
                            y="memory"
                            xScale={scaleTime()}
                            yDomain={[0, null]}
                            series={[
                                { key: "memory", color: "var(--color-memory)" },
                            ]}
                        >
                            {#snippet tooltip()}
                                <ChartTooltip />
                            {/snippet}
                        </AreaChart>
                    </ChartContainer>
                </div>
            </div>
        </div>
    {/if}

    <!-- Terminal Window -->
    <div
        class="flex-1 bg-black/95 rounded-lg border p-4 overflow-hidden flex flex-col shadow-inner min-h-[500px] max-h-[500px] overflow-y-scroll"
    >
        <div
            bind:this={logContainer}
            class="flex-1 overflow-y-auto font-mono text-sm text-zinc-300 space-y-1 pr-2"
        >
            {#if logs.length === 0}
                <div class="text-zinc-500 italic">
                    Waiting for server logs...
                </div>
            {/if}
            {#each logs as log}
                <div class="break-all hover:bg-white/5 px-1 rounded">{log}</div>
            {/each}
        </div>
    </div>
</main>
