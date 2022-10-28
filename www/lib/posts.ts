import fs from "fs";
import path from "path";
import matter from "gray-matter";
import { generateReadingTime } from "./helpers";
import { MDXRemoteSerializeResult } from "next-mdx-remote";

// substring amount for file names
// based on YYYY-MM-DD format
const FILENAME_SUBSTRING = 11;

const POST_DIRECTORY = path.join(process.cwd(), "_blog");

export interface Post {
  readonly slug?: string;
  readonly title: string;
  readonly date: string;
  readonly cover?: string;
  readonly caption?: string;
  readonly coverAspectRatio?: string;
  readonly author?: string;
  readonly author_url?: string;
  readonly excerpt?: string;
  readonly ogImage?: {
    readonly url: string;
  };
  readonly content?: MDXRemoteSerializeResult<Record<string, unknown>>;
  readonly toc?: MDXRemoteSerializeResult<Record<string, unknown>>;
  readonly thumb: string;
  readonly image?: string;
  readonly readingTime?: string;
  readonly description: string;
  readonly url: string;
  readonly tags?: string[];
  readonly logo?: string;
  readonly hideAuthor?: boolean;
}

export function getSortedPosts(
  limit?: number,
  tags?: readonly string[]
): readonly Post[] {
  //Reads all the files in the post directory
  const fileNames = fs.readdirSync(POST_DIRECTORY);

  // categories stored in this array

  let allPostsData: Post[] = fileNames.map((filename) => {
    const slug = filename.replace(".mdx", "");

    const fullPath = path.join(POST_DIRECTORY, filename);

    //Extracts contents of the MDX file
    const fileContents = fs.readFileSync(fullPath, "utf8");
    const { data, content } = matter(fileContents);
    const options: Intl.DateTimeFormatOptions = {
      month: "long",
      day: "numeric",
      year: "numeric",
    };
    const formattedDate = new Date(data.date).toLocaleDateString(
      "en-IN",
      options
    );

    const readingTime = generateReadingTime(content);

    // construct url to link to blog posts
    // based on datestamp in file name

    const dates = getDatesFromFileName(filename);
    let url = `${dates.year}/${dates.month}/${dates.day}/${slug.substring(
      FILENAME_SUBSTRING
    )}`;

    return {
      ...data,
      date: formattedDate,
      readingTime,
      url: url,
      slug,
    } as Post;
  });

  allPostsData = allPostsData.sort((a, b) => {
    if (new Date(a.date) < new Date(b.date)) {
      return 1;
    } else {
      return -1;
    }
  });

  if (tags) {
    allPostsData = allPostsData.filter((post) => {
      const found = tags.some((tag) => post.tags.includes(tag));
      return found;
    });
  }

  if (limit) allPostsData = allPostsData.slice(0, limit);

  return allPostsData;
}

// Get Slugs
export const getAllPostSlugs = () => {
  const fileNames = fs.readdirSync(POST_DIRECTORY);

  const files = fileNames.map((filename) => {
    const dates = getDatesFromFileName(filename);

    return {
      params: {
        ...dates,
        slug: filename.replace(".mdx", "").substring(FILENAME_SUBSTRING),
      },
    };
  });

  return files;
};

// Get Post based on Slug
export const getPostdata = async (slug: string) => {
  const fullPath = path.join(POST_DIRECTORY, `${slug}.mdx`);

  const postContent = fs.readFileSync(fullPath, "utf8");

  return postContent;
};

export function getAlltags(): readonly string[] {
  const posts = getSortedPosts();
  let tags: string[] = [];

  posts.map((post) => {
    post.tags.map((tag: string) => {
      if (!tags.includes(tag)) return tags.push(tag);
    });
  });

  return tags;
}

interface Dates {
  readonly year: string;
  readonly month: string;
  readonly day: string;
}

function getDatesFromFileName(filename: string): Dates {
  // extract YYYY, MM, DD from post name
  const year = filename.substring(0, 4);
  const month = filename.substring(5, 7);
  const day = filename.substring(8, 10);

  return {
    year,
    month,
    day,
  };
}
