const defaultTheme = require("tailwindcss/defaultTheme");

module.exports = {
  content: ["./{components,pages}/**/*.tsx"],
  theme: {
    extend: {
      colors: {
        "accent-1": "#fae15c",
        "accent-2": "#f25100",
        brand: {
          100: "#fae15c",
          200: "#fad149",
          300: "#fac138",
          400: "#f9b127",
          500: "#f9a016",
          600: "#f88e05",
          700: "#f67c00",
          800: "#f56800",
          900: "#f25100",
        },
        dark: {
          100: "#eeeeee",
          200: "#e0e0e0",
          300: "#bbbbbb",
          400: "#666666",
          500: "#444444",
          600: "#2a2a2a",
          700: "#1f1f1f",
          800: "#181818",
          900: "#0f0f0f",
        },
        gray: {
          100: "#eeeeee",
          200: "#e0e0e0",
          300: "#bbbbbb",
          400: "#7d7d7d",
          500: "#343434",
          600: "#2a2a2a",
          700: "#1f1f1f",
          800: "#181818",
          900: "#0f0f0f",
        },
      },
      fontFamily: {
        sans: ["Ubuntu", ...defaultTheme.fontFamily.sans],
        mono: ["Source Code Pro", "Menlo", "monospace"],
      },
    },
  },
  plugins: [],
};
