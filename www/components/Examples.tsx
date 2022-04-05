import ExternalLink from "./ExternalLink";

interface Card {
  readonly title: string;
  readonly description: string;
  readonly link: string;
  readonly icon: string;
}

const cards: Card[] = [
  {
    title: "Persist with Postgres",
    description:
      "Build any web service with a fully managed database using Rocket and sqlx",
    link: "https://github.com/getsynth/shuttle/tree/main/examples/rocket/postgres",
    icon: "/images/icon1.svg",
  },
  {
    title: "Url Shortener",
    description:
      "A URL shortener that you can use from your terminal - built with shuttle, rocket and postgres/sqlx.",
    link: "https://github.com/getsynth/shuttle/tree/main/examples/url-shortener",
    icon: "/images/icon2.svg",
  },
  {
    title: "JWT authentication",
    description:
      "Guard endpoints using self-issued JWT tokens while keeping public endpoint open",
    link: "https://github.com/getsynth/shuttle/tree/main/examples/rocket/authentication",
    icon: "/images/icon3.svg",
  },
];

export default function Examples() {
  return (
    <div
      id="examples"
      className="-mt-[122px] pt-[122px] lg:-mt-[66px] lg:pt-[66px]"
    >
      <div className="mx-auto max-w-6xl px-4 pt-16 pb-20 sm:px-6 lg:px-8 lg:pt-24 lg:pb-28">
        <div className="text-center">
          <h2 className="text-3xl font-extrabold tracking-tight text-gray-200 sm:text-4xl">
            From code to cloud in a minute
          </h2>
          <p className="mx-auto mt-3 max-w-2xl text-xl text-gray-300 sm:mt-4">
            Take your code to full-featured cloud infrastructure in under a
            minute. Don't take our word for it, see it for yourself.
          </p>
        </div>
        <div className="mx-auto  mt-12 grid w-fit gap-5 lg:grid-cols-3">
          {cards.map((card, index) => (
            <ExternalLink
              href={card.link}
              key={index}
              className="flex max-w-sm flex-col overflow-hidden rounded-lg transition hover:-translate-y-2 hover:shadow-2xl "
            >
              <div className="flex-shrink-0 bg-dark-800">
                <img
                  className="aspect-[4/3] w-full object-contain p-14"
                  src={card.icon}
                  role="presentation"
                />
              </div>
              <div className="flex flex-1 flex-col justify-between bg-gray-500 p-6">
                <div className="flex-1">
                  <div className="mt-2 block">
                    <p className="text-xl font-semibold text-gray-200">
                      {card.title}
                    </p>
                    <p className="mt-3 text-base text-gray-300">
                      {card.description}
                    </p>
                  </div>
                </div>
              </div>
            </ExternalLink>
          ))}
        </div>
      </div>
    </div>
  );
}
