// 复习计划日历（ECharts calendar 热力：颜色深浅=当日待复习数量）
import React, { useEffect, useMemo, useRef } from "react";
import * as echarts from "echarts";
import dayjs from "dayjs";
import type { ReviewPlan } from "../types";

interface ReviewCalendarProps {
  plans: ReviewPlan[];
  height?: number;
}

export default function ReviewCalendar({ plans, height = 220 }: ReviewCalendarProps): React.JSX.Element {
  const ref = useRef<HTMLDivElement>(null);
  const chartRef = useRef<echarts.ECharts | null>(null);

  // 按天聚合计数
  const { data, range, max } = useMemo(() => {
    const counts = new Map<string, number>();
    for (const p of plans) {
      const day = dayjs(p.nextDueAt).format("YYYY-MM-DD");
      counts.set(day, (counts.get(day) ?? 0) + 1);
    }
    const start = dayjs().subtract(7, "day").format("YYYY-MM-DD");
    const end = dayjs().add(45, "day").format("YYYY-MM-DD");
    const data = Array.from(counts.entries()).map(([date, count]) => [date, count]);
    const max = Math.max(1, ...counts.values());
    return { data, range: [start, end], max };
  }, [plans]);

  useEffect(() => {
    if (!ref.current) return;
    if (!chartRef.current) {
      chartRef.current = echarts.init(ref.current);
    }
    const chart = chartRef.current;
    chart.setOption({
      tooltip: { formatter: (p: { data: [string, number] }) => `${p.data[0]}：${p.data[1]} 项复习` },
      visualMap: {
        min: 0,
        max,
        orient: "horizontal",
        left: "center",
        bottom: 0,
        inRange: { color: ["#fff7ed", "#fdba74", "#ea580c"] },
        show: false,
      },
      calendar: {
        top: 24,
        left: 40,
        right: 16,
        cellSize: ["auto", 16],
        range,
        itemStyle: { borderWidth: 0.5 },
        splitLine: { show: false },
        dayLabel: { fontSize: 9 },
        monthLabel: { fontSize: 10 },
        yearLabel: { show: false },
      },
      series: [{ type: "heatmap", coordinateSystem: "calendar", data }],
    });

    const onResize = () => chart.resize();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [data, range, max]);

  useEffect(() => {
    return () => {
      chartRef.current?.dispose();
      chartRef.current = null;
    };
  }, []);

  return <div ref={ref} style={{ width: "100%", height }} />;
}
