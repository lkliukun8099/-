// 掌握度曲线（ECharts 折线图）
import React, { useEffect, useRef } from "react";
import * as echarts from "echarts";
import type { CurvePoint } from "../types";

interface MasteryChartProps {
  points: CurvePoint[];
  height?: number;
}

export default function MasteryChart({ points, height = 280 }: MasteryChartProps): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);

  useEffect(() => {
    if (!ref.current) return;
    if (!chartRef.current) {
      chartRef.current = echarts.init(ref.current);
    }
    const chart = chartRef.current;
    chart.setOption({
      grid: { left: 48, right: 16, top: 24, bottom: 32 },
      tooltip: { trigger: "axis" },
      xAxis: {
        type: "category",
        data: points.map((p) => p.date),
        axisLabel: { fontSize: 10 },
      },
      yAxis: {
        type: "value",
        min: 0,
        max: 1,
        axisLabel: { formatter: (v: number) => `${(v * 100).toFixed(0)}%` },
      },
      series: [
        {
          name: "平均掌握度",
          type: "line",
          smooth: true,
          symbolSize: 6,
          data: points.map((p) => Number(p.avgScore.toFixed(3))),
          areaStyle: { opacity: 0.15 },
          lineStyle: { color: "#ea580c" },
          itemStyle: { color: "#ea580c" },
        },
      ],
    });

    const onResize = () => chart.resize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [points]);

  useEffect(() => {
    return () => {
      chartRef.current?.dispose();
      chartRef.current = null;
    };
  }, []);

  return <div ref={ref} style={{ width: "100%", height }} />;
}
