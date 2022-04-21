import { CheckIcon } from "@heroicons/react/outline";

const features = [
  {
    name: "Infrastructure from Rust",
    description:
      "Traits and annotations define all your infrastructure from your service code",
  },
  {
    name: "Serverless",
    description:
      "All the infrastructure and wiring you need is handled by the deployer runtime. No VMs or containers",
  },
  {
    name: "Databases",
    description:
      "Wiring up a service to a persistent database is as easy as deriving a trait",
  },
  {
    name: "Entirely open-source",
    description:
      "A completely free and open-source project, with no vendor lock-in or hidden code",
  },
  {
    name: "Self-hosting friendly",
    description:
      "Deploy the entire platform under your own AWS account for extra security and isolation",
  },
  {
    name: "Fast deploy times",
    description:
      "Deploy new versions as quickly as running an incremental build, all with zero downtime",
  },
];

export default function Features() {
  return (
    <div
      id="features"
      className="-mt-[122px] pt-[122px] lg:-mt-[66px] lg:pt-[66px]"
    >
      <div className="mx-auto max-w-6xl py-16 px-4 sm:px-6 lg:py-24 lg:px-8">
        <div className="mx-auto max-w-3xl text-center">
          <h2 className="text-3xl font-extrabold tracking-tight dark:text-gray-200 sm:text-4xl">
            Serverless for Rust
          </h2>
          <p className="mt-4 text-xl text-slate-500 dark:text-gray-300">
            The only serverless platform that lets you control your
            infrastructure from Rust code as easily as deriving a trait.
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
                <p className="ml-9 text-lg font-medium leading-6 dark:text-gray-200">
                  {feature.name}
                </p>
              </dt>
              <dd className="mt-2 ml-9 text-base text-slate-500 dark:text-gray-300">
                {feature.description}
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </div>
  );
}
