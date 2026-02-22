<script>
  import { ChartContainer, ChartTooltip } from "$lib/components/ui/chart";
  import { AreaChart } from "layerchart";
  let data = [{ time: 1, cpu: 10 }, { time: 2, cpu: 20 }];
</script>
<ChartContainer config={{ cpu: { label: "CPU", color: "hsl(var(--chart-1))" } }}>
  <AreaChart {data} x="time" y="cpu" series={[{ key: 'cpu', color: 'var(--color-cpu)' }]}>
    {#snippet tooltip()}
      <ChartTooltip />
    {/snippet}
  </AreaChart>
</ChartContainer>
