import React from 'react';

interface SparklineProps {
  data: number[];
  color?: string;
  width?: number;
  height?: number;
  fill?: boolean;
}

export function Sparkline({
  data,
  color = "currentColor",
  width = 84,
  height = 22,
  fill = true
}: SparklineProps) {
  const reactId = React.useId();
  if (!data || !data.length) return null;
  const min = Math.min(...data);
  const max = Math.max(...data);
  const range = max - min;
  // When every value is identical, no line is meaningful — render only the
  // terminal dot so the user sees a stable marker instead of a flat line.
  if (range === 0) {
    return (
      <svg width={width} height={height} className="overflow-visible">
        <circle cx={width - 2} cy={height / 2} r="2" fill={color} />
      </svg>
    );
  }
  const stepX = width / (data.length - 1);
  const pts = data.map((v, i) => [
    i * stepX,
    height - ((v - min) / range) * (height - 4) - 2
  ]);
  const d = pts.map((p, i) => (i === 0 ? `M${p[0]},${p[1]}` : `L${p[0]},${p[1]}`)).join(" ");
  const area = `${d} L${width},${height} L0,${height} Z`;
  // useId() guarantees a unique gradient <defs> per Sparkline instance, so
  // multiple sparklines with the same data no longer share a gradient and
  // render in the wrong color.
  const gid = `vox-spark-${reactId}`;

  return (
    <svg width={width} height={height} className="overflow-visible">
      <defs>
        <linearGradient id={gid} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.35" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      {fill && <path d={area} fill={`url(#${gid})`} />}
      <path d={d} fill="none" stroke={color} strokeWidth="1.25" strokeLinecap="round" strokeLinejoin="round" />
      <circle cx={pts[pts.length - 1][0]} cy={pts[pts.length - 1][1]} r="2" fill={color} />
    </svg>
  );
}
