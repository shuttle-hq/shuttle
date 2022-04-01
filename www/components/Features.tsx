import { CheckIcon } from "@heroicons/react/outline";

const features = [
  {
    name: "Invite team members",
    description:
      "Tempor tellus in aliquet eu et sit nulla tellus. Suspendisse est, molestie blandit quis ac. Lacus.",
  },
  {
    name: "Notifications",
    description:
      "Ornare donec rhoncus vitae nisl velit, neque, mauris dictum duis. Nibh urna non parturient.",
  },
  {
    name: "List view",
    description:
      "Etiam cras augue ornare pretium sit malesuada morbi orci, venenatis. Dictum lacus.",
  },
  {
    name: "Boards",
    description:
      "Interdum quam pulvinar turpis tortor, egestas quis diam amet, natoque. Mauris sagittis.",
  },
  {
    name: "Keyboard shortcuts",
    description:
      "Ullamcorper in ipsum ac feugiat. Senectus at aliquam vulputate mollis nec. In at risus odio.",
  },
  {
    name: "Reporting",
    description:
      "Magna a vel sagittis aliquam eu amet. Et lorem auctor quam nunc odio. Sed bibendum.",
  },
];

export default function Features() {
  return (
    <div className="mx-auto max-w-6xl py-16 px-4 sm:px-6 lg:py-24 lg:px-8">
      <div className="mx-auto max-w-3xl text-center">
        <h2 className="text-3xl font-extrabold text-gray-200">
          All-in-one platform
        </h2>
        <p className="mt-4 text-lg text-gray-300">
          Ac euismod vel sit maecenas id pellentesque eu sed consectetur.
          Malesuada adipiscing sagittis vel nulla nec.
        </p>
      </div>
      <dl className="mt-12 space-y-10 sm:grid sm:grid-cols-2 sm:gap-x-6 sm:gap-y-12 sm:space-y-0 lg:grid-cols-3 lg:gap-x-8">
        {features.map((feature) => (
          <div key={feature.name} className="relative">
            <dt>
              <CheckIcon
                className="absolute h-6 w-6 text-green-500"
                aria-hidden="true"
              />
              <p className="ml-9 text-lg font-medium leading-6 text-gray-200">
                {feature.name}
              </p>
            </dt>
            <dd className="mt-2 ml-9 text-base text-gray-300">
              {feature.description}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
