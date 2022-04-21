import "../styles/index.css";
import type { AppProps } from "next/app";
import React, { useEffect } from "react";
import { useRouter } from "next/router";
import Head from "next/head";
import { DefaultSeo } from "next-seo";
import { setupMixpanel } from "../lib/mixpanel";
import {
  APP_NAME,
  SITE_TITLE,
  SITE_DESCRIPTION,
  SITE_URL,
  TWITTER_HANDLE,
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

config.autoAddCss = false;

export default function App({ Component, pageProps }: AppProps) {
  const router = useRouter();
  useEffect(() => setupMixpanel(router));
  const { user } = pageProps;

  return (
    <UserProvider user={user}>
      <ApiKeyModalStateProvider>
        <AnnouncementBarIsClosedProvider>
          <Head>
            <title>{SITE_TITLE}</title>
          </Head>
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
