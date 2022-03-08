const defaultTheme = require('tailwindcss/defaultTheme')

module.exports = {
    purge: [],
    darkMode: false, // or 'media' or 'class'
    theme: {
        extend: {
            screens: {
                sm: '640px',
                // => @media (min-width: 640px) { ... }

                md: '768px',
                // => @media (min-width: 768px) { ... }

                lg: '1024px',
                // => @media (min-width: 1024px) { ... }

                xl: '1280px',
                // => @media (min-width: 1280px) { ... }

                '2xl': '1536px',
                // => @media (min-width: 1536px) { ... }
            },
            colors: {
                'accent-1': '#fae15c',
                'accent-2': '#f25100',
                brand: {
                    100: '#fae15c',
                    200: '#fad149',
                    300: '#fac138',
                    400: '#f9b127',
                    500: '#f9a016',
                    600: '#f88e05',
                    700: '#f67c00',
                    800: '#f56800',
                    900: '#f25100',
                },
                dark: {
                    100: '#eeeeee',
                    200: '#e0e0e0',
                    300: '#bbbbbb',
                    400: '#666666',
                    500: '#444444',
                    600: '#2a2a2a',
                    700: '#1f1f1f',
                    800: '#181818',
                    900: '#0f0f0f',
                },
                gray: {
                    100: '#eeeeee',
                    200: '#e0e0e0',
                    300: '#bbbbbb',
                    400: '#7d7d7d',
                    500: '#343434',
                    600: '#2a2a2a',
                    700: '#1f1f1f',
                    800: '#181818',
                    900: '#0f0f0f',
                },
            },
            spacing: {
                28: '7rem',
            },
            letterSpacing: {
                tighter: '-.04em',
            },
            lineHeight: {
                tight: 1.2,
            },
            fontSize: {
                '5xl': '2.5rem',
                '6xl': '2.75rem',
                '7xl': '4.5rem',
                '8xl': '6.25rem',
            },
            boxShadow: {
                small: '0 5px 10px rgba(0, 0, 0, 0.12)',
                medium: '0 8px 30px rgba(0, 0, 0, 0.12)',
            },
            fontFamily: {
                sans: [
                    'Inter',
                    ...defaultTheme.fontFamily.sans
                ],
                mono: ['Source Code Pro', 'Menlo', 'monospace'],
            },
            stroke: (theme) => ({
                white: theme('colors.white'),
                black: theme('colors.black'),
            }),
        },
        fontFamily: {
            'Gilroy': ['Gilroy']
        }
    },
    variants: {
        extend: {},
    },
    plugins: [],
}
