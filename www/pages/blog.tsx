import { useMemo, useState } from "react";
import { useRouter } from "next/router";
import Image from "next/image";
import { NextSeo } from "next-seo";
import { getAlltags, getSortedPosts, Post } from "../lib/posts";
import { getAuthors } from "../lib/authors";
import BlogListItem from "../components/blog/BlogListItem";
import { SITE_URL } from "../lib/constants";
import { GetStaticPropsResult } from "next";
import InternalLink from "../components/InternalLink";
import classnames from "classnames";

export async function getStaticProps(): Promise<GetStaticPropsResult<Props>> {
  const allPostsData = getSortedPosts();
  const tags = getAlltags();

  return {
    props: {
      blogs: allPostsData,
      tags,
    },
  };
}

interface Props {
  readonly blogs: ReturnType<typeof getSortedPosts>;
  readonly tags: readonly string[];
}

export default function Blog(props: Props): JSX.Element {
  const tags = ["all", ...props.tags];
  const [activeTag, setActiveTag] = useState("all");
  const router = useRouter();

  const [headPost, tailPosts] = useMemo(() => {
    const [head, ...tail] = props.blogs;

    return [head, tail];
  }, [props.blogs]);

  const blogs = useMemo(() => {
    if (activeTag === "all") return tailPosts;

    return tailPosts.filter((post) => post.tags.includes(activeTag));
  }, [tailPosts, activeTag]);

  const meta_title = "Shuttle Blog";
  const meta_description = "Get all your shuttle News on the shuttle blog.";

  return (
    <>
      <NextSeo
        title={meta_title}
        description={meta_description}
        openGraph={{
          title: meta_title,
          description: meta_description,
          url: SITE_URL + router.pathname,
          images: [
            {
              url: `${SITE_URL}images/og/og-image.jpg`,
            },
          ],
        }}
        additionalLinkTags={[
          {
            rel: "alternate",
            type: "application/rss+xml",
            href: `${SITE_URL}rss.xml`,
          },
        ]}
      />

      <div className="overflow-hidden py-12">
        <div className="mx-auto mt-16 max-w-6xl px-4 sm:px-6 lg:px-8">
          <div className="mx-auto">
            <FeaturedThumb post={headPost} />
          </div>
        </div>
      </div>

      <div className="border-t dark:border-gray-400">
        <div className="mx-auto max-w-6xl px-4 sm:px-6 lg:px-8">
          <div className="mx-auto ">
            <div className="grid grid-cols-12">
              <div className="col-span-12 lg:col-span-12">
                <div className="mb-4">
                  <div className="sm:hidden">
                    <label htmlFor="tags" className="sr-only">
                      Select a tag
                    </label>

                    <select
                      id="tags"
                      name="tags"
                      className="block w-full rounded dark:bg-gray-600 dark:text-gray-300"
                      defaultValue={activeTag}
                      onChange={(e) => void setActiveTag(e.target.value)}
                    >
                      {tags.map((tag, index) => (
                        <option key={index} value={index}>
                          {tag}
                        </option>
                      ))}
                    </select>
                  </div>
                  <div className="hidden overflow-x-scroll py-2 sm:block">
                    <nav className="flex space-x-4" aria-label="Tabs">
                      {tags.map((tag, index) => (
                        <button
                          key={index}
                          onClick={() => void setActiveTag(tag)}
                          className={classnames(
                            "z-10 cursor-pointer rounded px-3 py-2 text-sm font-medium hover:shadow-md",
                            {
                              "bg-brand-orange2 text-white": activeTag === tag,
                              "text-slate-700 hover:bg-slate-200 hover:text-slate-700 dark:text-gray-300 hover:dark:bg-gray-600 hover:dark:text-gray-200":
                                activeTag !== tag,
                            }
                          )}
                          aria-current={activeTag === tag ? "page" : undefined}
                        >
                          {tag}
                        </button>
                      ))}
                    </nav>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <ol className="grid grid-cols-12 py-16 lg:gap-16">
            {blogs.map((blog, index) => (
              <div
                className="col-span-12 lg:col-span-6 xl:col-span-4"
                key={index}
              >
                <BlogListItem post={blog} />
              </div>
            ))}
          </ol>
        </div>
      </div>
    </>
  );
}

interface FeaturedThumbProps {
  readonly post: Post;
}

function FeaturedThumb({ post }: FeaturedThumbProps) {
  const author = getAuthors(post.author?.split(",") ?? []);

  return (
    <div key={post.slug} className="w-full cursor-pointer">
      <InternalLink
        href={`/blog/${post.url}`}
        className="grid gap-8 lg:grid-cols-2 lg:gap-16"
      >
        <div className="relative aspect-[4/3] w-full overflow-auto rounded">
          <Image
            src={`/images/blog/` + post.thumb}
            layout="fill"
            objectFit="cover"
          />
        </div>
        <div className="flex flex-col space-y-2">
          <div className="flex space-x-2 text-sm text-slate-500 dark:text-gray-400">
            <p>{post.date}</p>
            <p>•</p>
            <p>{post.readingTime}</p>
          </div>

          <div>
            <h2 className="mb-4 text-3xl">{post.title}</h2>
            <p className="mb-4 text-xl text-slate-600 dark:text-gray-300">
              {post.description}
            </p>
          </div>

          <div className="grid w-max grid-flow-col grid-rows-4 gap-4">
            {author.map((author, index) => {
              return (
                <div className="flex items-center space-x-3" key={index}>
                  {author.author_image_url && (
                    <div className="relative h-10 w-10 overflow-auto">
                      <Image
                        src={author.author_image_url}
                        alt={`${author.author} avatar`}
                        className="rounded-full"
                        layout="fill"
                        objectFit="cover"
                      />
                    </div>
                  )}
                  <div className="flex flex-col">
                    <span className="m-0 text-sm dark:text-gray-200">
                      {author.author}
                    </span>
                    <span className="m-0 text-xs text-slate-500 dark:text-gray-400">
                      {author.position}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </InternalLink>
    </div>
  );
}
