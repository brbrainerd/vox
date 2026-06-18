import { useCallback } from 'react';
import { useLocalStorage } from './useLocalStorage';

export interface MetricPoint {
  t: number;
  v: number;
}

export function metricSeriesStorageKey(key: string): string {
  return `vox.metric.series.v1.${key}`;
}

/** Append a sample and trim to the rolling window capacity. */
export function appendMetricSample(
  series: MetricPoint[],
  value: number,
  maxPoints = 60,
): MetricPoint[] {
  const lastT = series.length > 0 ? series[series.length - 1].t + 1 : 0;
  const next = [...series, { t: lastT, v: value }];
  return next.length > maxPoints ? next.slice(-maxPoints) : next;
}

/** Convert a sparkline value array into chart-ready `{ t, v }` points. */
export function metricSeriesFromSpark(spark: number[]): MetricPoint[] {
  return spark.map((v, i) => ({ t: i, v }));
}

/** When spark is populated, widgets seed from it; otherwise append on value changes. */
export function shouldAppendMetricFromKpi(
  spark: number[],
  value: number,
  previousValue: number | undefined,
): boolean {
  if (spark.length > 0) {
    return false;
  }
  return previousValue !== value;
}

export function useMetricSeries(key: string, initial: MetricPoint[]) {
  const storageKey = metricSeriesStorageKey(key);
  const [series, setSeries] = useLocalStorage<MetricPoint[]>(storageKey, initial);

  const append = useCallback(
    (value: number, maxPoints = 60) => {
      setSeries((prev) => appendMetricSample(prev, value, maxPoints));
    },
    [setSeries],
  );

  return { series, setSeries, append };
}
