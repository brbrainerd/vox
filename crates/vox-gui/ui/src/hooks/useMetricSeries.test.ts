import { describe, it, expect } from 'vitest';
import {
  appendMetricSample,
  metricSeriesFromSpark,
  shouldAppendMetricFromKpi,
  type MetricPoint,
} from './useMetricSeries';

describe('useMetricSeries helpers', () => {
  it('appendMetricSample caps series at max points', () => {
    const seed: MetricPoint[] = [
      { t: 0, v: 1 },
      { t: 1, v: 2 },
      { t: 2, v: 3 },
    ];
    const capped = appendMetricSample(seed, 99, 3);
    expect(capped).toHaveLength(3);
    expect(capped[0]).toEqual({ t: 1, v: 2 });
    expect(capped[1]).toEqual({ t: 2, v: 3 });
    expect(capped[2]).toEqual({ t: 3, v: 99 });
  });

  it('metricSeriesFromSpark converts spark array to chart points', () => {
    expect(metricSeriesFromSpark([10, 20, 30])).toEqual([
      { t: 0, v: 10 },
      { t: 1, v: 20 },
      { t: 2, v: 30 },
    ]);
  });

  it('shouldAppendMetricFromKpi prefers spark samples over append', () => {
    expect(shouldAppendMetricFromKpi([1, 2, 3], 5, undefined)).toBe(false);
    expect(shouldAppendMetricFromKpi([1, 2, 3], 5, 4)).toBe(false);
  });

  it('shouldAppendMetricFromKpi appends when spark is empty and value changed', () => {
    expect(shouldAppendMetricFromKpi([], 5, undefined)).toBe(true);
    expect(shouldAppendMetricFromKpi([], 5, 3)).toBe(true);
    expect(shouldAppendMetricFromKpi([], 5, 5)).toBe(false);
  });
});
