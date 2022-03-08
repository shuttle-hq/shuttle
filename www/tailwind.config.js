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
                'accent-1': '#00dab8',
                'accent-2': '#008c98',
                brand: {
                    100: '#00ffba',
                    200: '#00eab9',
                    300: '#00dbb8',
                    400: '#00dab8',
                    500: '#00c2b7',
                    600: '#00b1a6',
                    700: '#009c9f',
                    800: '#00969c',
                    900: '#008c98',
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
