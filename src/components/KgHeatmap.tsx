// 知识薄弱热力图（ECharts treemap：面积=作答次数，颜色=掌握度，红=薄弱）
import React, { useEffect, useRef } from "react";
import * as echarts from "echarts";
import type { HeatCell } from "../types";

interface KgHeatmapProps {
  cells: HeatCell[];
  height?: number;
}

function scoreColor(score: number): string {
  // 0 → 红，0.5 → 黄，1 → 绿
  const r = score < 0.5 ? 220 : Math.round(220 - (score - 0.5) * 2 * 180);
  const g = score < 0.5 ? Math.round(60 + score * 2 * 140) : 200;
  return `rgb(${r},${g},80)`;
}

export default function KgHeatmap({ cells, height = 360 }: KgHeatmapProps): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    if (!chartRef.current) {
      chartRef.current = echarts.init(ref.current);
    }
    const chart = chartRef.current;
    chart.setOption({
      tooltip: {
        formatter: (info: { data: { pathText: string; score: number; attempts: number } }) =>
          `${info.data.pathText}<br/>掌握度：${(info.data.score * 100).toFixed(0)}% · 作答 ${info.data.attempts} 次`,
      },
      series: [
        {
          type: "treemap",
          roam: false,
          nodeClick: false,
          breadcrumb: { show: false },
          label: { show: true, formatter: "{b}", fontSize: 11 },
          itemStyle: { borderColor: "#fff", borderWidth: 1 },
          data: cells.map((c) => ({
            name: c.nodeName,
            value: Math.max(1, c.attempts),
            score: c.score,
            attempts: c.attempts,
            pathText: c.nodePath.join(" → ") || c.nodeName,
            itemStyle: { color: scoreColor(c.score) },
          })),
        },
      ],
    });

    const onResize = () => chart.resize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [cells]);

  useEffect(() => {
    return () => {
      chartRef.current?.dispose();
      chartRef.current = null;
    };
  }, []);

  return <div ref={ref} style={{ width: "100%", height }} />;
}
