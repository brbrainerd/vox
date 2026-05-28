export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        void: '#09090b',
        steel: '#71717a',
        brass: '#d4af37',
        "amber-glow": 'rgba(212,175,55,0.5)',
        border: 'rgba(255,255,255,0.06)',
        background: '#09090b',
        primary: '#d4af37',
      },
      fontFamily: {
        display: ['Outfit', 'Inter', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },
      animation: {
        'vox-ping': 'vox-ping 2s cubic-bezier(0, 0, 0.2, 1) infinite',
        'vox-shimmer': 'vox-shimmer 2.5s infinite linear',
        'vox-toast-in': 'vox-toast-in 0.4s cubic-bezier(0.16, 1, 0.3, 1)',
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
      },
    },
  },
};
