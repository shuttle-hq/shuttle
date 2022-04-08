import type { NextApiRequest, NextApiResponse } from "next";
import { getSortedPosts } from "../../lib/posts";
import { generateRss } from "../../lib/rss";

export default function handler(
  req: NextApiRequest,
  res: NextApiResponse<string>
) {
  const allPostsData = getSortedPosts();
  const rss = generateRss(allPostsData);

  res.setHeader("Content-Type", "application/xml");

  res.status(200).send(rss);
}
