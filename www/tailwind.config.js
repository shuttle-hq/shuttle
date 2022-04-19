const defaultTheme = require("tailwindcss/defaultTheme");

module.exports = {
  content: ["./{components,pages}/**/*.tsx"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        "brand-yellow1": "#fae15c",
        "brand-orange1": "#ff8a3f",
        "brand-orange2": "#f25100",
        "brand-purple1": "#7777DD",
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
      typography: ({ theme }) => ({
        toc: {
          css: {
            ul: {
              "list-style-type": "none",
              "padding-left": 0,
              margin: 0,
              li: {
                "padding-left": 0,
              },
              a: {
                display: "block",
                "text-decoration": "none",
                fontSize: "0.8rem",
                fontWeight: "200",
                color: theme("colors.gray[300]"),
                "&:hover": {
                  color: theme("colors.gray[200]"),
                },
                "font-weight": "400",
              },
              // margin: 0,
              ul: {
                "list-style-type": "none",
                li: {
                  marginTop: "0.2rem",
                  marginBottom: "0.2rem",
                  "padding-left": "0 !important",
                  "margin-left": "0.5rem",
                },
                a: {
                  fontWeight: "200",
                  color: theme("colors.scale[400]"),
                  "&:hover": {
                    color: theme("colors.scale[200]"),
                  },
                },
              },
            },
          },
        },
      }),
    },
  },
  plugins: [
    require("@tailwindcss/typography"),
    require("@tailwindcss/forms"),
    // require('@tailwindcss/line-clamp'),
    // require('@tailwindcss/aspect-ratio'),
  ],
};
