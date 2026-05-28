/** @type {import('tailwindcss').Config} */
module.exports = {
  darkMode: ["class"],
  content: [
    './webview-ui/src/**/*.{ts,tsx,js,jsx}',
  ],
  theme: {
    extend: {
      colors: {
        background: "var(--vox-bg-void)",
        foreground: "var(--vox-steel)",
        card: {
          DEFAULT: "var(--vox-bg-surface)",
          foreground: "var(--vox-steel)",
        },
        popover: {
          DEFAULT: "var(--vox-bg-elevated)",
          foreground: "var(--vox-steel)",
        },
        primary: {
          DEFAULT: "var(--vox-amber)",
          foreground: "#000000",
        },
        secondary: {
          DEFAULT: "var(--vox-bg-machine)",
          foreground: "var(--vox-brass)",
        },
        muted: {
          DEFAULT: "var(--vox-bg-machine)",
          foreground: "var(--vox-steel)",
        },
        accent: {
          DEFAULT: "var(--vox-cyan)",
          foreground: "#000000",
        },
        destructive: {
          DEFAULT: "#EF4444",
          foreground: "#FFFFFF",
        },
        border: "var(--vox-brass)",
        input: "var(--vox-bg-machine)",
        ring: "var(--vox-cyan)",
      },
      borderRadius: {
        lg: "4px",
        md: "2px",
        sm: "0px",
      },
    },
  },
  plugins: [],
}
