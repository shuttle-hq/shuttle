import React from "react";
import {
  faG,
  faGlasses,
  faGlobe,
  faHeadset,
  faHeart,
  faLightbulb,
  faRocket,
} from "@fortawesome/free-solid-svg-icons";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import {
  CalendarIcon,
  ChevronRightIcon,
  LocationMarkerIcon,
  TerminalIcon,
  UsersIcon,
} from "@heroicons/react/solid";
import { motion, useViewportScroll, useTransform } from "framer-motion";
import FadeIn from "../components/FadeIn";

const positions = [
  {
    id: 1,
    title: "Rust Software Engineer",
    type: "Full-time",
    location: "Remote",
    experience: "3+ years",
  },
  {
    id: 2,
    title: "Front End Developer",
    type: "Full-time",
    location: "Remote",
    experience: "2+ years",
  },
  {
    id: 3,
    title: "User Interface Designer",
    type: "Full-time",
    location: "Remote",
    experience: "4+ years",
  },
];

const features = [
  {
    name: "We build in public.",
    description: "Our community helps us build a better Lightdash.",
    icon: faRocket,
  },
  {
    name: "We believe in autonomy.",
    description: "‍Only the best ideas win here.",
    icon: faLightbulb,
  },
  {
    name: "We are remote-first.",
    description: "Our culture is built around this.",
    icon: faGlobe,
  },
  {
    name: "We love open source!",
    description:
      "We believe it will play a massive part in the future of data tools.",
    icon: faHeart,
  },
  {
    name: "We bias towards impact.",
    description:
      "‍We’d rather build something to 80% and get it in front of users so we can iterate fast!",
    icon: faGlasses,
  },
  {
    name: "We have incredible support.",
    description:
      "‍From our community to top investors & angels including Y Combinator.",
    icon: faHeadset,
  },
];

export default function company() {
  return (
    <>
      <div className="mx-auto max-w-6xl px-4 pt-16 pb-20 sm:px-6 lg:px-8 lg:pt-24 lg:pb-28">
        <div className="text-center">
          <h2 className="text-base font-semibold uppercase tracking-wider text-brand-orange2">
            Our Mission
          </h2>

          <p className="mt-2 text-3xl font-extrabold tracking-tight dark:text-gray-200  sm:text-4xl">
            Company Vision
          </p>
          <p className="mx-auto mt-5 max-w-prose text-xl dark:text-gray-300">
            Lorem ipsum dolor, sit amet consectetur adipisicing elit. Magni
            eveniet neque possimus amet veritatis sapiente exercitationem soluta
            et dolore animi beatae explicabo pariatur illum aut, repellat
            numquam sunt architecto eaque.
          </p>
        </div>
      </div>

      <div className="mx-auto max-w-6xl px-4 pt-16 pb-32 sm:px-6 lg:px-8 lg:pt-24 lg:pb-40">
        <div className="text-center">
          <p className="mt-2 text-3xl font-extrabold tracking-tight dark:text-gray-200  sm:text-4xl">
            Company Values
          </p>
          <p className="mx-auto mt-5 max-w-prose text-xl dark:text-gray-300">
            Lorem ipsum dolor, sit amet consectetur adipisicing elit. Magni
            eveniet neque possimus amet veritatis sapiente exercitationem soluta
            et dolore animi beatae explicabo pariatur illum aut, repellat
            numquam sunt architecto eaque.
          </p>
          <div className="mt-12">
            <div className="grid grid-cols-1 gap-8 sm:grid-cols-2 lg:grid-cols-3">
              {features.map((feature) => (
                <FadeIn key={feature.name} className="pt-6">
                  <div className="flow-root h-full rounded-lg bg-slate-200 px-6 pb-8 transition hover:-translate-y-2 hover:shadow-2xl dark:bg-gray-600">
                    <div className="-mt-6">
                      <div>
                        <span className="inline-flex items-center justify-center rounded-md bg-brand-orange1 p-3 shadow-lg">
                          <FontAwesomeIcon
                            icon={feature.icon}
                            className="h-6 w-6 text-white"
                          />
                        </span>
                      </div>
                      <h3 className="mt-8 text-lg font-medium tracking-tight dark:text-gray-200 ">
                        {feature.name}
                      </h3>
                      <p className="mt-5 text-base dark:text-gray-300">
                        {feature.description}
                      </p>
                    </div>
                  </div>
                </FadeIn>
              ))}
            </div>
          </div>
        </div>
      </div>

      <div className="mx-auto max-w-6xl px-4 pt-16 pb-20 sm:px-6 lg:px-8 lg:pt-24 lg:pb-28">
        <div className="mb-6 text-center">
          <p className="mt-2 text-3xl font-extrabold tracking-tight dark:text-gray-200  sm:text-4xl">
            Our Contributors Get Paid
          </p>
          <p className="mx-auto mt-5 max-w-prose text-xl dark:text-gray-300">
            Lorem ipsum dolor, sit amet consectetur adipisicing elit. Magni
            eveniet neque possimus amet veritatis sapiente exercitationem soluta
            et dolore animi beatae explicabo pariatur illum aut, repellat
            numquam sunt architecto eaque.
          </p>
        </div>
      </div>

      <div className="mx-auto max-w-6xl px-4 pt-16 pb-20 sm:px-6 lg:px-8 lg:pt-24">
        <div className="text-center">
          <p className="mt-2 text-3xl font-extrabold tracking-tight dark:text-gray-200  sm:text-4xl">
            It's time to build, join us!
          </p>
          <p className="mx-auto mt-5 max-w-prose text-xl dark:text-gray-300">
            Interested? We're hiring! Check out our live roles below:
          </p>
        </div>
      </div>
      <div className="mx-auto max-w-3xl px-4 pb-20 sm:px-6 lg:px-8 lg:pb-28">
        <div className="overflow-hidden bg-slate-200 shadow dark:bg-gray-600 sm:rounded-md">
          <ul
            role="list"
            className="divide-y divide-gray-200 dark:divide-gray-700"
          >
            {positions.map((position) => (
              <li key={position.id}>
                <a href="#" className="block hover:bg-black/5">
                  <div className="py-4 px-6">
                    <div className="flex items-center justify-between gap-2">
                      <div className="flex w-full flex-col justify-between gap-2 sm:flex-row">
                        <p className="truncate text-base font-medium dark:text-gray-200">
                          {position.title}
                        </p>

                        <div className="flex items-center gap-2 text-base dark:text-gray-300">
                          <div>{position.type}</div>
                          <div>|</div>
                          <div>{position.location}</div>
                          <div>|</div>
                          <div>{position.experience}</div>
                        </div>
                      </div>
                      <div>
                        <ChevronRightIcon
                          className="h-5 w-5 dark:text-gray-300"
                          aria-hidden="true"
                        />
                      </div>
                    </div>
                  </div>
                </a>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </>
  );
}
