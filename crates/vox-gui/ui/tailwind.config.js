export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        void: '#09090b',
        steel: '#71717a',
        // Accent palette is theme-switched via the --brass CSS var (see index.css).
        // <alpha-value> keeps existing opacity utilities (brass/40, …) working.
        brass: 'rgb(var(--brass) / <alpha-value>)',
        "amber-glow": 'rgb(var(--brass) / 0.5)',
        border: 'rgba(255,255,255,0.06)',
        background: '#09090b',
        primary: 'rgb(var(--brass) / <alpha-value>)',
        // Semantic tokens (Style Dictionary → tokens.generated.css):
        'bg-base': 'var(--color-bg-base)',
        'bg-surface': 'var(--color-bg-surface)',
        'bg-elevated': 'var(--color-bg-elevated)',
        'text-primary': 'var(--color-text-primary)',
        'text-secondary': 'var(--color-text-secondary)',
        'text-muted': 'var(--color-text-muted)',
        'border-subtle': 'var(--color-border-subtle)',
        'border-strong': 'var(--color-border-strong)',
        'accent': 'var(--color-accent-default)',
      },
      fontFamily: {
        display: ['Outfit', 'Inter', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      animation: {
        'vox-ping': 'vox-ping 2s cubic-bezier(0, 0, 0.2, 1) infinite',
        'vox-shimmer': 'vox-shimmer 2.5s infinite linear',
        'vox-toast-in': 'vox-toast-in 0.4s cubic-bezier(0.16, 1, 0.3, 1)',
        shimmer: 'shimmer 1.5s ease-in-out infinite',
      },
      keyframes: {
        'vox-ping': {
          '75%, 100%': { transform: 'scale(2.5)', opacity: '0' },
        },
        'vox-shimmer': {
          '0%': { transform: 'translateX(-100%)' },
          '100%': { transform: 'translateX(100%)' },
        },
        'vox-toast-in': {
          '0%': { transform: 'translateX(24px)', opacity: '0' },
          '100%': { transform: 'translateX(0)', opacity: '1' },
        },
        shimmer: {
          '0%': { backgroundPosition: '200% 0' },
          '100%': { backgroundPosition: '-200% 0' },
        },
      },
    },
  },
};
