import { useForm, ValidationError } from "@formspree/react";
import {
  CONTRIBUTING_URL,
  DISCORD_URL,
  FORMSPREE_ENDPOINT,
  GITHUB_URL,
  TWITTER_URL,
} from "../lib/constants";
import ExternalLink from "./ExternalLink";
import Image from "next/image";
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
        <p className="mt-3 text-lg text-slate-500 dark:text-gray-300 sm:mt-4">
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
      <div
        id="code-snippets"
        className="mx-auto max-w-6xl px-4 pt-5 pb-20 sm:px-6 lg:px-8 lg:pt-4 lg:pb-4"
      >
        <div className="grid w-full grid-cols-1 gap-4 lg:grid-cols-12">
          <div className="mb-4 max-w-lg lg:col-span-5">
            <p className="mt-3 text-left text-lg text-slate-500 dark:text-gray-300 sm:mt-4">
              Lorem ipsum dolor.
            </p>
            <p className="mt-3 text-left text-lg text-slate-500 dark:text-gray-300 sm:mt-4">
              Lorem ipsum dolor sit amet, consectetur adipiscing elit. Nulla a
              placerat nisi. Mauris quis congue purus. Nullam orci est, laoreet
              venenatis mi quis, consectetur semper ligula. Sed id augue
              bibendum, finibus felis eget, ultrices eros. Donec feugiat sed
              lacus id faucibus. Cras vitae efficitur lectus.
            </p>
          </div>
          <div className="lg:col-span-7">
            <div className="relative top-1/2 mx-auto max-w-xs -translate-y-1/2">
              <Image
                src={"/images/fuse.gif"}
                layout="responsive"
                height="100%"
                width="100%"
              />
            </div>
          </div>
        </div>
      </div>
      <form onSubmit={handleSubmit} className="align-center flex flex-col">
        <input
          id="email"
          type="email"
          name="email"
          placeholder={`hello@shuttle.rs`}
          className="text-m mt-3 block w-full max-w-sm self-center rounded rounded border border-gray-300 bg-slate-300 p-4 text-slate-700 text-gray-900 hover:border-brand-orange1 focus:border-brand-orange1 focus:ring-brand-orange1 dark:border-gray-600 dark:bg-gray-500 dark:text-white dark:placeholder-gray-400 dark:focus:border-brand-orange1 dark:focus:ring-brand-orange1"
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
