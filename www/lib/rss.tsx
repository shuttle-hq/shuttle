import { APP_NAME, SITE_URL } from "./constants";
import { Post } from "./posts";

const generateRssItem = (post): string => `
<item>
  <guid>${SITE_URL}blog/${post.url}</guid>
  <title>${post.title}</title>
  <link>${SITE_URL}blog/${post.url}</link>
  <description>${post.description}</description>
  <pubDate>${new Date(post.date).toUTCString()}</pubDate>
</item>
`;

export const generateRss = (posts: readonly Post[]): string => `
  <rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
    <channel>
      <title>Blog - ${APP_NAME}</title>
      <link>${SITE_URL}</link>
      <description>Latest news from ${APP_NAME}</description>
      <language>en</language>
      <lastBuildDate>${new Date(posts[0].date).toUTCString()}</lastBuildDate>
      <atom:link href="${SITE_URL}blog/rss.xml" rel="self" type="application/rss+xml"/>
      ${posts.map(generateRssItem).join("")}
    </channel>
  </rss>
`;
