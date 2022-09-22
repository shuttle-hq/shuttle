import { useRouter } from "next/router";
import Code from "./Code";
import {
  DISCORD_URL,
  SHUTTLE_DOCS_URL,
  SITE_DESCRIPTION,
  SITE_TITLE,
} from "../lib/constants";
import classnames from "classnames";
import { useAnnouncementBarIsClosed } from "./AnnouncementBar";
import ExternalLink from "./ExternalLink";

export default function Hero() {
  const { basePath } = useRouter();
  const [announcementBarIsClosed] = useAnnouncementBarIsClosed();

  return (
    <div
      className={classnames(
        "flex w-full flex-col justify-center dark:bg-dark-700",
        {
          "min-h-[calc(100vh-107px)]": !announcementBarIsClosed,
          "min-h-[calc(100vh-75px)]": announcementBarIsClosed,
        }
      )}
    >
      <div className="mx-auto py-5 xl:px-12">
        <div className="p-6 sm:py-8">
          <div className="m-auto flex max-w-5xl flex-col gap-8 text-center sm:gap-11">
            {/* <div className="flex m-auto relative">
              <img
                className="h-16"
                src={`${basePath}/images/logo.png`}
                alt="Shuttle"
              />
              <span className="dark:bg-brand-orange1 dark:text-dark-700 font-bold absolute scale-[.8] bottom-[-26px] right-[-5px] text-base px-[10px] py-[2px] rounded">
                ALPHA
              </span>
            </div> */}

            <div>
              <div className="mb-5 text-4xl font-bold dark:text-gray-200 sm:text-5xl md:text-6xl">
                {SITE_TITLE}
              </div>
              <div className="px-10 text-xl text-slate-500 dark:text-gray-300 ">
                {SITE_DESCRIPTION}
              </div>
            </div>
            <div className="hidden flex-col items-center justify-center md:flex">
              <Code
                id="cargo-install-cargo-shuttle"
                code="cargo install cargo-shuttle"
              />
            </div>

            <div className="flex justify-center gap-4">
              <ExternalLink
                className="rounded bg-brand-900 py-3 px-8 font-bold text-white transition hover:bg-brand-700"
                href={SHUTTLE_DOCS_URL}
                target="_self"
                // mixpanelEvent="Get Started"
              >
                Get Started
              </ExternalLink>

              <ExternalLink
                className="rounded bg-brand-purple1 py-3 px-8 font-bold text-white transition hover:brightness-125"
                href={DISCORD_URL}
                // mixpanelEvent="Join Discord"
              >
                Join Discord
              </ExternalLink>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
