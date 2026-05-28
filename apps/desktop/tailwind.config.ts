import type { Config } from 'tailwindcss';

export default {
  darkMode: ['class'],
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  theme: {
    extend: {
      colors: {
        // shadcn/ui semantic tokens
        background: 'hsl(var(--background))',
        foreground: 'hsl(var(--foreground))',
        card: 'hsl(var(--card))',
        'card-foreground': 'hsl(var(--card-foreground))',
        popover: 'hsl(var(--popover))',
        'popover-foreground': 'hsl(var(--popover-foreground))',
        border: 'hsl(var(--border))',
        input: 'hsl(var(--input))',
        ring: 'hsl(var(--ring))',
        muted: 'hsl(var(--muted))',
        'muted-foreground': 'hsl(var(--muted-foreground))',
        accent: 'hsl(var(--accent))',
        'accent-foreground': 'hsl(var(--accent-foreground))',
        primary: 'hsl(var(--primary))',
        'primary-foreground': 'hsl(var(--primary-foreground))',
        secondary: 'hsl(var(--secondary))',
        'secondary-foreground': 'hsl(var(--secondary-foreground))',
        destructive: 'hsl(var(--destructive))',
        'destructive-foreground': 'hsl(var(--destructive-foreground))',
        sidebar: {
          DEFAULT: 'hsl(var(--sidebar-background))',
          foreground: 'hsl(var(--sidebar-foreground))',
          accent: 'hsl(var(--sidebar-accent))',
          'accent-foreground': 'hsl(var(--sidebar-accent-foreground))',
          border: 'hsl(var(--sidebar-border))',
          primary: 'hsl(var(--sidebar-primary))',
          'primary-foreground': 'hsl(var(--sidebar-primary-foreground))',
        },

        // ── Glassmorphism design system tokens ──────────────────
        // These mirror the HTML mockup's Tailwind config directly.
        'surface': '#0b1326',
        'on-surface': '#dae2fd',
        'surface-container-lowest': '#060e20',
        'surface-container-low': '#131b2e',
        'surface-container': '#171f33',
        'surface-container-high': '#222a3d',
        'surface-container-highest': '#2d3449',
        'on-surface-variant': '#bdc8d1',
        'outline': '#87929a',
        'outline-variant': '#3e484f',
        'primary-glass': '#8ed5ff',
        'primary-container': '#38bdf8',
        'on-primary': '#00354a',
        'on-primary-container': '#004965',
        'secondary-container': '#2f3aa3',
        'on-secondary-container': '#a8afff',
        'tertiary': '#45e3ce',
        'tertiary-container': '#07c7b2',
        'on-tertiary-container': '#004d44',
      },
      fontFamily: {
        sans: ['Geist', '"SF Pro Display"', 'ui-sans-serif', 'system-ui'],
        mono: ['"Geist Mono"', 'ui-monospace', 'monospace'],
      },
      boxShadow: {
        panel: '0 18px 60px rgba(0, 0, 0, 0.35)',
        subtle: '0 8px 30px rgba(0, 0, 0, 0.22)',
        'primary-glow': '0 0 20px rgba(142, 213, 255, 0.15)',
        'tertiary-glow': '0 0 8px rgba(69, 227, 206, 0.5)',
      },
      borderRadius: {
        xl: 'calc(var(--radius) - 2px)',
        '2xl': 'var(--radius)',
        '3xl': 'calc(var(--radius) + 6px)',
      },
      keyframes: {
        'accordion-down': {
          from: { height: '0' },
          to: { height: 'var(--radix-accordion-content-height)' },
        },
        'accordion-up': {
          from: { height: 'var(--radix-accordion-content-height)' },
          to: { height: '0' },
        },
      },
      animation: {
        'accordion-down': 'accordion-down 0.2s ease-out',
        'accordion-up': 'accordion-up 0.2s ease-out',
      },
    },
  },
  plugins: [],
} satisfies Config;
