import { useForm, ValidationError } from "@formspree/react";
import {
  CONTRIBUTING_URL,
  DISCORD_URL,
  FORMSPREE_ENDPOINT,
  GITHUB_URL,
  TWITTER_URL,
} from "../lib/constants";
import ExternalLink from "./ExternalLink";

const links = [
  { name: "💻 contributing to shuttle", href: CONTRIBUTING_URL },
  { name: "⭐️ starring the repository", href: GITHUB_URL },
  { name: "👾 joining our discord community", href: DISCORD_URL },
  { name: "🐦 following us on twitter", href: TWITTER_URL },
];

export default function SignupForm() {
  const [state, handleSubmit] = useForm(FORMSPREE_ENDPOINT);

  if (state.succeeded) {
    return (
      <div className="mb-4 lg:col-span-5">
        <p className="mt-3 text-lg text-slate-500 dark:text-gray-300 sm:mt-4">
          Thank you for registering your interest in the next iteration of
          shuttle.
        </p>
        <p className="text-lg text-slate-500 dark:text-gray-300">
          If you are looking for a way to support shuttle in the meantime, you
          can do so by:
        </p>
        <div className="mt-3 flex flex-col">
          {links.map((link) => (
            <ExternalLink
              key={link.name}
              href={link.href}
              className="mt-3 text-lg text-slate-600 hover:text-slate-900 dark:text-gray-200 hover:dark:text-white"
            >
              {link.name}
            </ExternalLink>
          ))}
        </div>
      </div>
    );
  }
  return (
    <>
      <div className="mt-5">
        <div className="mb-7 text-4xl font-bold dark:text-gray-200 sm:text-5xl md:text-6xl">
          Build Backends. Fast.
        </div>
        <div className="mt-4 px-10 text-xl text-slate-500 dark:text-gray-300">
          Sign up to our closed beta and get updated on the{" "}
          {
            <ExternalLink
              key="shuttle-next"
              href="https://shuttle.rs/blog/2022/10/21/shuttle-beta"
              className="text-brand-orange1 hover:text-brand-orange2"
            >
              next iteration of shuttle
            </ExternalLink>
          }
          , a serverless backend framework with the fastest build, test and
          deploy times ever.
        </div>
      </div>
      <form onSubmit={handleSubmit} className="align-center flex flex-col">
        <input
          id="email"
          type="email"
          name="email"
          placeholder={`hello@shuttle.rs`}
          className="text-m mt-2 block w-full max-w-sm self-center rounded border border-gray-300 bg-slate-300 p-4 text-slate-700  hover:border-brand-orange1 focus:border-brand-orange1 focus:ring-brand-orange1 dark:border-gray-600 dark:bg-gray-500 dark:text-white dark:placeholder-gray-400 dark:focus:border-brand-orange1 dark:focus:ring-brand-orange1"
        />
        <ValidationError prefix="Email" field="email" errors={state.errors} />
        <button
          type="submit"
          className="mt-6 self-center rounded bg-brand-900 py-3 px-8 font-bold text-white transition hover:bg-brand-700"
          disabled={state.submitting}
        >
          Sign Up
        </button>
      </form>
    </>
  );
}
