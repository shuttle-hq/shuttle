import "../styles/index.css";
import type { AppProps } from "next/app";
import React, { useEffect } from "react";
import { useRouter } from "next/router";
import Head from "next/head";
import { DefaultSeo } from "next-seo";
import { setupMixpanel } from "../lib/helpers";
import {
  APP_NAME,
  SITE_TITLE,
  SITE_DESCRIPTION,
  SITE_URL,
  TWITTER_HANDLE,
} from "../lib/constants";
import AnnouncementBar from "../components/AnnouncementBar";
// import Footer from "../components/Footer";

function GetSynthed({ Component, pageProps }: AppProps) {
  const router = useRouter();
  useEffect(() => setupMixpanel(router));

  return (
    <>
      <Head>
        <title>{SITE_TITLE}</title>
      </Head>

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

      <div className="text-dark-200 full bg-dark-600">
        <AnnouncementBar />
        <Component {...pageProps} />
        {/* <Footer /> */}
      </div>
    </>
  );
}

export default GetSynthed;
