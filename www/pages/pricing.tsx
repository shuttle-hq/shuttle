import React from "react";
import { Fragment } from "react";
import classnames from "classnames";
import { DISCORD_URL } from "../lib/constants";
import ExternalLink from "../components/ExternalLink";
import LoginButton from "../components/LoginButton";

const tiers = [
  {
    name: "Hobby",
    BuyButton: LoginButton,
    price: (
      <>
        <span className="text-4xl font-extrabold dark:text-gray-200">$0</span>{" "}
        <span className="text-base font-medium dark:text-gray-300">/mo</span>
      </>
    ),
    description:
      "The perfect confluence of features to run your hobby-projects for free - forever.",
  },
  {
    name: "Pro",
    BuyButton() {
      const label = "Contact Us";

      return (
        <ExternalLink
          className="inline-block w-full rounded border border-slate-900 bg-transparent py-1 px-4 text-center text-base font-medium text-slate-900 transition-colors hover:bg-slate-800 hover:text-slate-100 dark:border-white dark:text-white hover:dark:bg-white hover:dark:text-dark-700"
          href="mailto:hello@shuttle.rs"
        >
          {label}
        </ExternalLink>
      );
    },
    price: (
      <span className="text-4xl font-extrabold dark:text-gray-200">
        Let's Talk
      </span>
    ),
    description:
      "Build on production quality infrastructure which scales to your needs.",
  },
];

const sections = [
  {
    name: "Features",
    features: [
      {
        name: "Team Size",
        tiers: { Hobby: 1, Pro: "Get in touch" },
      },
      {
        name: "Deployments",
        tiers: { Hobby: "Unlimited", Pro: "Unlimited" },
      },
      {
        name: "Number of Projects",
        tiers: { Hobby: 5, Pro: "Get in touch" },
      },
      {
        name: "Requests",
        tiers: { Hobby: "150K/mo", Pro: "Get in touch" },
      },
      {
        name: "Workers",
        tiers: { Hobby: 1, Pro: "Get in touch" },
      },
      {
        name: "Database Storage",
        tiers: { Hobby: "500 MB", Pro: "Get in touch" },
      },
      {
        name: "Subdomains",
        tiers: { Hobby: "1 Per Project", Pro: "1 Per Project" },
      },
      {
        name: "Custom Domains",
        tiers: { Hobby: "N/A", Pro: "1 Per Project" },
      },
    ],
  },
  {
    name: "Support",
    features: [
      {
        name: "Community",
        tiers: {
          Hobby: (
            <ExternalLink
              href={DISCORD_URL}
              className="text-slate-600 underline hover:text-slate-900 dark:text-gray-200 hover:dark:text-white"
            >
              Discord
            </ExternalLink>
          ),
          Pro: (
            <ExternalLink
              href={DISCORD_URL}
              className="text-slate-600 underline hover:text-slate-900 dark:text-gray-200 hover:dark:text-white"
            >
              Discord
            </ExternalLink>
          ),
        },
      },
      {
        name: "Request Turnaround",
        tiers: { Hobby: "N/A", Pro: "24 hr" },
      },
    ],
  },
];

export default function Pricing() {
  return (
    <div className="dark:bg-dark-700 dark:text-gray-200">
      <div className="mx-auto max-w-6xl py-16 px-4 sm:py-24 sm:px-6 lg:px-8">
        {/* xs to lg */}
        <div className="mx-auto max-w-2xl space-y-16 lg:hidden">
          {tiers.map((tier, tierIdx) => (
            <section key={tier.name}>
              <div className="mb-8 px-4">
                <h2 className="text-lg font-medium leading-6 dark:text-gray-200">
                  {tier.name}
                </h2>
                <p className="mt-4">{tier.price}</p>
                <p className="mt-4 mb-4 text-sm dark:text-gray-300">
                  {tier.description}
                </p>
                <tier.BuyButton />
              </div>

              {sections.map((section) => (
                <table key={section.name} className="w-full">
                  <caption className="border-t bg-slate-200 py-3 px-4 text-left text-sm font-medium dark:border-dark-500 dark:bg-gray-500 dark:text-gray-200">
                    {section.name}
                  </caption>
                  <thead>
                    <tr>
                      <th className="sr-only" scope="col">
                        Feature
                      </th>
                      <th className="sr-only" scope="col">
                        Included
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-dark-500">
                    {section.features.map((feature) => (
                      <tr
                        key={feature.name}
                        className="border-t dark:border-dark-500"
                      >
                        <th
                          className="py-5 px-4 text-left text-sm font-normal dark:text-gray-300"
                          scope="row"
                        >
                          {feature.name}
                        </th>
                        <td className="py-5 pr-4">
                          <span className="block text-right text-sm">
                            {feature.tiers[tier.name]}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ))}

              <div
                className={classnames(
                  tierIdx < tiers.length - 1 ? "border-b py-5" : "pt-5",
                  "border-t px-4 dark:border-dark-500"
                )}
              >
                <tier.BuyButton />
              </div>
            </section>
          ))}
        </div>

        {/* lg+ */}
        <div className="hidden lg:block">
          <table className="h-px w-full table-fixed">
            <caption className="sr-only">Pricing plan comparison</caption>
            <thead>
              <tr>
                <th
                  className={`w-1/${
                    tiers.length + 1
                  } px-6 pb-4 text-left text-sm font-medium dark:text-gray-200`}
                  scope="col"
                >
                  <span className="sr-only">Feature by</span>
                  <span>Plans</span>
                </th>
                {tiers.map((tier) => (
                  <th
                    key={tier.name}
                    className={`w-1/${
                      tiers.length + 1
                    } px-6 pb-4 text-left text-lg font-medium leading-6 dark:text-gray-200`}
                    scope="col"
                  >
                    {tier.name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody className="divide-y divide-dark-500 border-t dark:border-dark-500">
              <tr>
                <th
                  className="py-8 px-6 text-left align-top text-sm font-medium dark:text-gray-200"
                  scope="row"
                >
                  Pricing
                </th>
                {tiers.map((tier) => (
                  <td key={tier.name} className="h-full py-8 px-6 align-top">
                    <div className="relative table h-full">
                      <p>{tier.price}</p>
                      <p className="mt-4 mb-4 text-sm dark:text-gray-300">
                        {tier.description}
                      </p>
                      <tier.BuyButton />
                    </div>
                  </td>
                ))}
              </tr>
              {sections.map((section) => (
                <Fragment key={section.name}>
                  <tr>
                    <th
                      className="bg-slate-200 py-3 pl-6 text-left text-sm font-medium dark:bg-gray-500 dark:text-gray-200"
                      colSpan={tiers.length + 1}
                      scope="colgroup"
                    >
                      {section.name}
                    </th>
                  </tr>
                  {section.features.map((feature) => (
                    <tr key={feature.name}>
                      <th
                        className="py-5 px-6 text-left text-sm font-normal dark:text-gray-300"
                        scope="row"
                      >
                        {feature.name}
                      </th>
                      {tiers.map((tier) => (
                        <td key={tier.name} className="py-5 px-6">
                          <span className="block text-sm">
                            {feature.tiers[tier.name]}
                          </span>
                        </td>
                      ))}
                    </tr>
                  ))}
                </Fragment>
              ))}
            </tbody>
            <tfoot>
              <tr className="border-t dark:border-dark-500">
                <th className="sr-only" scope="row">
                  Choose your plan
                </th>
                {tiers.map((tier) => (
                  <td key={tier.name} className="px-6 pt-5">
                    <tier.BuyButton />
                  </td>
                ))}
              </tr>
            </tfoot>
          </table>
        </div>
      </div>
    </div>
  );
}
