import { useRouter } from "next/router";
import Code from "./Code";
import { SITE_DESCRIPTION, SITE_TITLE } from "../lib/constants";
import mixpanel from "mixpanel-browser";

const Hero = () => {
  const { basePath } = useRouter();
  return (
    <div className="w-full min-h-screen -mt-8 flex flex-col justify-center bg-dark-700">
      <div className="xl:px-12 py-5 mx-auto">
        <div className="p-6 sm:py-8">
          <div className="max-w-3xl m-auto text-center flex flex-col gap-8 sm:gap-11">
            <div className="flex m-auto relative">
              <img
                className="h-16"
                src={`${basePath}/images/logo.png`}
                alt="Shuttle"
              />
              <span className="bg-brand-orange1 text-dark-700 font-bold absolute scale-[.8] bottom-[-26px] right-[-5px] text-base px-[10px] py-[2px] rounded">
                ALPHA
              </span>
            </div>

            <div>
              <div className="mb-5 text-4xl sm:text-5xl md:text-6xl font-bold text-gray-200">
                {SITE_TITLE}
              </div>
              <div className="text-xl font-normal text-gray-300 px-10">
                {SITE_DESCRIPTION}
              </div>
            </div>
            <div className="hidden md:flex flex-col justify-center items-center">
              <Code code="cargo install cargo-shuttle" />
            </div>

            <div className="flex gap-4 justify-center">
              <a
                ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                className="text-white font-bold bg-brand-900 hover:bg-brand-700 py-3 px-8 rounded transition"
                href="https://docs.rs/shuttle-service/latest/shuttle_service/"
              >
                Get Started
              </a>

              <a
                ref={(el) => el && mixpanel.track_links(el, `Clicked Link`)}
                className="text-white font-bold bg-brand-purple1 hover:brightness-125 py-3 px-8 rounded transition"
                href="https://discord.gg/H33rRDTm3p"
                target="_blank"
              >
                Join Discord
              </a>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Hero;
