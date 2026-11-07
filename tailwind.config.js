/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{ts,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // Semantic cockpit palette — resolved from CSS variables so the three
        // themes (dark / midnight / sepia) swap without touching components.
        app: 'var(--app-bg)',
        panel: 'var(--panel)',
        panel2: 'var(--panel-2)',
        panel3: 'var(--panel-3)',
        line: 'var(--line)',
        line2: 'var(--line-2)',
        ink: 'var(--ink)',
        mut: 'var(--muted)',
        accent: 'var(--accent)',
        accent2: 'var(--accent-2)',
        ok: 'var(--ok)',
        warn: 'var(--warn)',
        bad: 'var(--bad)',
      },
      fontFamily: {
        sans: [
          '-apple-system',
          'BlinkMacSystemFont',
          'Segoe UI',
          'Roboto',
          'Helvetica Neue',
          'Arial',
          'sans-serif',
        ],
        literary: [
          'Iowan Old Style',
          'Palatino Linotype',
          'Palatino',
          'Book Antiqua',
          'Georgia',
          'serif',
        ],
        mono: [
          'ui-monospace',
          'SFMono-Regular',
          'JetBrains Mono',
          'Menlo',
          'Consolas',
          'monospace',
        ],
      },
      fontSize: {
        '2xs': ['11px', '14px'],
      },
    },
  },
  plugins: [],
};
