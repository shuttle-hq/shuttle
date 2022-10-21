import { useRouter } from "next/router";
import SignupForm from "./SignupForm";

const BetaHero = () => {
  const { basePath } = useRouter();
  return (
    <div className="-mt-8 flex min-h-screen w-full flex-col justify-center dark:bg-dark-700">
      <div className="mx-auto py-5 xl:px-12">
        <div className="p-6 sm:py-5">
          <div className="m-auto flex max-w-3xl flex-col gap-4 text-center sm:gap-10">
            <div className="relative m-auto flex">
              <img
                className="h-20 w-auto"
                src={`${basePath}/images/logo.png`}
                alt="Shuttle"
              />
              <span className="absolute bottom-[-26px] right-[-5px] scale-[.8] rounded bg-brand-orange1 px-[10px] py-[2px] text-base font-bold text-slate-100 dark:text-dark-700">
                BETA
              </span>
            </div>
            <div className="mt-8 mb-2 max-w-xl">
              <h2 className="text-3xl font-extrabold tracking-tight dark:text-gray-200 sm:text-3xl">
                Fastest backend development experience ever
              </h2>
              <p className="mt-6 text-lg text-slate-500 dark:text-gray-300 sm:mt-6">
                A next-generation backend framework with the fastest build, test
                and deployment times ever.
              </p>
            </div>
            <SignupForm />
          </div>
        </div>
      </div>
    </div>
  );
};

export default BetaHero;
