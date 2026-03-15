/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,ts,svelte}'],
  theme: {
    extend: {
      colors: {
        teal:  { DEFAULT: '#1B7A8A', dark: '#155f6e' },
        peach: { DEFAULT: '#E8956A', dark: '#d4784c' },
        amber: { DEFAULT: '#D4A017' },
        cream: { DEFAULT: '#F9F5F0' },
      },
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', 'Segoe UI', 'sans-serif'],
      },
    },
  },
  plugins: [],
}
