import React from 'react';

export function Backdrop() {
  return (
    <>
      <div className="pointer-events-none fixed inset-0 -z-10 bg-[#09090b]" />
      <div className="pointer-events-none fixed inset-0 -z-10 opacity-[0.18] [background-image:linear-gradient(to_right,rgba(255,255,255,0.06)_1px,transparent_1px),linear-gradient(to_bottom,rgba(255,255,255,0.06)_1px,transparent_1px)] [background-size:48px_48px]" />
      <div className="pointer-events-none fixed inset-0 -z-10 [background:radial-gradient(900px_500px_at_18%_-10%,rgba(212,175,55,0.10),transparent_60%),radial-gradient(900px_500px_at_82%_110%,rgba(139,92,246,0.10),transparent_60%),radial-gradient(700px_400px_at_50%_50%,rgba(34,211,238,0.05),transparent_70%)]" />
      <div className="pointer-events-none fixed inset-0 -z-10 mix-blend-overlay opacity-[0.06] [background:repeating-linear-gradient(0deg,rgba(255,255,255,0.4)_0,rgba(255,255,255,0.4)_1px,transparent_1px,transparent_3px)]" />
    </>
  );
}
