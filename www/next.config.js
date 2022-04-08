module.exports = {
  i18n: {
    locales: ["en"],
    defaultLocale: "en",
  },
  images: {
    domains: ["github.com"],
  },
  async rewrites() {
    return [
      {
        source: "/rss.xml",
        destination: "/api/rss",
      },
    ];
  },
};
