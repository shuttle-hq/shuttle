import "../styles/index.css";
import type { AppProps } from "next/app";
import React, { useEffect } from "react";
import { useRouter } from "next/router";
import Head from "next/head";
import { DefaultSeo } from "next-seo";
import {
  APP_NAME,
  SITE_TITLE,
  SITE_DESCRIPTION,
  SITE_URL,
  TWITTER_HANDLE,
  GA_MEASUREMENT_ID,
} from "../lib/constants";
import AnnouncementBar, {
  AnnouncementBarIsClosedProvider,
} from "../components/AnnouncementBar";
import { UserProvider } from "@auth0/nextjs-auth0";
import ApiKeyModal, {
  ApiKeyModalStateProvider,
} from "../components/ApiKeyModal";
import Footer from "../components/Footer";
import Header from "../components/Header";
import { config } from "@fortawesome/fontawesome-svg-core";
import Script from "next/script";
import { setupGoogleAnalytics } from "../lib/gtag";

config.autoAddCss = false;

export default function App({ Component, pageProps }: AppProps) {
  const router = useRouter();
  useEffect(() => setupGoogleAnalytics(router));
  const { user } = pageProps;

  return (
    <UserProvider user={user}>
      <ApiKeyModalStateProvider>
        <AnnouncementBarIsClosedProvider>
          <Head>
            <title>{SITE_TITLE}</title>
          </Head>
          <Script
            id="krunchdata-analytics"
            strategy="afterInteractive"
            dangerouslySetInnerHTML={{
              __html: `
              var script = document.createElement("script"); 
              script.src = "https://app.krunchdata.io/assets/js/k.js"; 
              script.dataset.api = "https://app.krunchdata.io/traffic/web/record"; 
              script.dataset.id = "+pPt5ByH17wHAsiBQt81sT2mcnCKbAJT1ERg9+IRMFfedUlpkU+m/jRF1/TppjZl";
              document.head.appendChild(script); 
              console.log("added Krunch script to head");
              `,
            }}
          />
          {/* Global Site Tag (gtag.js) - Google Analytics */}
          <Script
            strategy="afterInteractive"
            src={`https://www.googletagmanager.com/gtag/js?id=${GA_MEASUREMENT_ID}`}
          />
          <Script
            id="google-analytics"
            strategy="afterInteractive"
            dangerouslySetInnerHTML={{
              __html: `
            window.dataLayer = window.dataLayer || [];
            function gtag(){dataLayer.push(arguments);}
            gtag('js', new Date());

            gtag('config', '${GA_MEASUREMENT_ID}', {
              page_path: window.location.pathname,
            });
          `,
            }}
          />
          <Script
            strategy="afterInteractive"
            dangerouslySetInnerHTML={{
              __html: `
                (function(d, w) {
                  w.MissiveChatConfig = {
                    "id": "35790ffd-9049-42c8-a9d4-e0865535419c"
                  };

                  var s = d.createElement('script');
                  s.async = true;
                  s.src = 'https://webchat.missiveapp.com/' + w.MissiveChatConfig.id + '/missive.js';
                  if (d.head) d.head.appendChild(s);
                })(document, window);
              `,
            }}
          />
          <DefaultSeo
            title={APP_NAME}
            description={SITE_DESCRIPTION}
            openGraph={{
              type: "website",
              url: SITE_URL,
              site_name: APP_NAME,
            }}
            twitter={{
              handle: TWITTER_HANDLE,
              site: TWITTER_HANDLE,
              cardType: "summary_large_image",
            }}
          />

          <div className="min-h-screen bg-slate-100 text-slate-800 dark:bg-dark-700 dark:text-dark-200">
            <AnnouncementBar />
            <Header />
            <Component {...pageProps} />
            <ApiKeyModal />
            <Footer />
          </div>
        </AnnouncementBarIsClosedProvider>
      </ApiKeyModalStateProvider>
    </UserProvider>
  );
}
