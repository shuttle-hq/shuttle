import { APP_NAME, SITE_URL } from "./constants";
import { getSortedPosts, Post } from "./posts";
import fs from "fs";

const postXML = (post: Post): string => `
<item>
  <guid>${SITE_URL}blog/${post.url}</guid>
  <title>${post.title}</title>
  <link>${SITE_URL}blog/${post.url}</link>
  <description>${post.description}</description>
  <pubDate>${new Date(post.date).toUTCString()}</pubDate>
</item>
`;

const rssXML = (posts: readonly Post[]): string => `
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>Blog - ${APP_NAME}</title>
    <link>${SITE_URL}</link>
    <description>Latest news from ${APP_NAME}</description>
    <language>en</language>
    <lastBuildDate>${new Date(posts[0].date).toUTCString()}</lastBuildDate>
    <atom:link href="${SITE_URL}/rss.xml" rel="self" type="application/rss+xml"/>
    ${posts.map(postXML).join("")}
  </channel>
</rss>
`;

const posts = getSortedPosts();

fs.writeFile("./public/rss.xml", rssXML(posts), (err) => {
  if (err != null) {
    console.error(err);
    process.exit(1);
  }
});
