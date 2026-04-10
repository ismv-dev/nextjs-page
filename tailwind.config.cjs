/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    './app/**/*.{js,ts,jsx,tsx}',
    './app/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      colors: {
        primary: '#04128e',
        secondary: '#1e3a8a',
        accent: '#3b82f6',
        'dark-bg': '#10101d',
        'dark-card': 'rgba(255, 255, 255, 0.05)',
        'dark-input': 'rgba(255, 255, 255, 0.08)',
        'games-panel': '#bdc3c7',
        'games-border': '#ecf0f1',
        'games-cell': '#bdc3c7',
        'games-revealed': '#1e272e',
        'mine-hit': 'rgba(255, 48, 96, 0.25)',
        'mine-shown': 'rgba(255, 48, 96, 0.08)',
      },
      fontFamily: {
        jersey: ['Jersey 10', 'sans-serif'],
      },
      boxShadow: {
        'soft': '0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06)',
      },
      gridTemplateColumns: {
        'sudoku': 'repeat(9, 40px)',
        'minesweeper': 'repeat(auto-fit, minmax(34px, 1fr))',
      },
    },
  },
  darkMode: 'class',
  plugins: [],
}
