import Document, { Html, Head, Main, NextScript } from 'next/document'

export default class MyDocument extends Document {
    render() {
        return (
            <Html lang="en" className="bg-dark-600">
                <Head></Head>
                <body>
                <Main />
                <NextScript />
                </body>
            </Html>
        )
    }
}