import matter from "gray-matter";
import { getAuthors } from "../../../../../lib/authors";
import { serialize } from "next-mdx-remote/serialize";
import { NextSeo } from "next-seo";
import Image from "next/image";
import { useRouter } from "next/router";
import React from "react";
import { generateReadingTime } from "../../../../../lib/helpers";
import {
  getAllPostSlugs,
  getPostdata,
  getSortedPosts,
  Post,
} from "../../../../../lib/posts";
import { MDXRemote, MDXRemoteProps } from "next-mdx-remote";
import gfm from "remark-gfm";
import slug from "rehype-slug";
import toc from "markdown-toc";
import rehypePrism from "@mapbox/rehype-prism";
import { SITE_URL } from "../../../../../lib/constants";
import { GetStaticPropsContext, GetStaticPropsResult } from "next";
import { ParsedUrlQuery } from "querystring";
import InternalLink from "../../../../../components/InternalLink";
import ExternalLink from "../../../../../components/ExternalLink";
import classnames from "classnames";
import { DocumentTextIcon } from "@heroicons/react/outline";
import { FontAwesomeIcon } from "@fortawesome/react-fontawesome";
import { faLinkedin, faTwitter } from "@fortawesome/free-brands-svg-icons";
import Copy from "../../../../../components/Copy";
import Socials from "../../../../../components/Socials";

export async function getStaticPaths() {
  const paths = getAllPostSlugs();
  return {
    paths,
    fallback: false,
  };
}

interface Params extends ParsedUrlQuery {
  readonly year: string;
  readonly month: string;
  readonly day: string;
  readonly slug: string;
}

export async function getStaticProps({
  params,
}: GetStaticPropsContext<Params>): Promise<GetStaticPropsResult<Props>> {
  const filePath = `${params.year}-${params.month}-${params.day}-${params.slug}`;
  const postContent = await getPostdata(filePath);
  const readingTime = generateReadingTime(postContent);
  const { data, content } = matter(postContent);

  const mdxPost = await serialize(content, {
    scope: data,
    mdxOptions: {
      remarkPlugins: [gfm],
      rehypePlugins: [slug, rehypePrism],
    },
  });

  const contentTOC = toc(content, {
    maxdepth: data.toc_depth ?? 3,
  });

  const mdxTOC = await serialize(contentTOC.content);

  const relatedPosts = getSortedPosts(
    6,
    mdxPost.scope.tags as readonly string[]
  )
    .filter((p) => p.slug != filePath)
    .slice(0, 5);

  const allPosts = getSortedPosts();

  const currentIndex = allPosts
    .map(function (e) {
      return e.slug;
    })
    .indexOf(filePath);

  const nextPost = allPosts[currentIndex + 1] ?? null;
  const prevPost = allPosts[currentIndex - 1] ?? null;

  const options: Intl.DateTimeFormatOptions = {
    month: "long",
    day: "numeric",
    year: "numeric",
  };
  const formattedDate = new Date(data.date).toLocaleDateString(
    "en-IN",
    options
  );

  return {
    props: {
      prevPost,
      nextPost,
      relatedPosts,
      blog: {
        slug: `${params.year}/${params.month}/${params.day}/${params.slug}`,
        content: mdxPost,
        ...data,
        toc: mdxTOC,
        readingTime,
        date: formattedDate,
      } as Post,
    },
  };
}

const Pre = ({ children, ...props }: any) => {
  let line = 1;

  const code = React.useMemo(() => {
    return [children.props.children]
      .flat()
      .flatMap((child) => {
        if (typeof child !== "string") {
          return child.props.children;
        } else {
          return child;
        }
      })
      .join("");
  }, [children]);

  return (
    <div className="relative">
      <Copy code={code}></Copy>

      <pre {...props} className={props.className ?? "language-"}>
        {{
          ...children,
          props: {
            ...children.props,
            className: children.props.className ?? "language-",
            children: [
              <span className="mr-4 inline-block w-4 select-none text-right italic text-[rgb(92,99,112)] last:hidden">
                {line}
              </span>,
              ...[children.props.children].flat().flatMap((child) => {
                if (typeof child === "string") {
                  const [head, ...tail] = child.split("\n");
                  return [
                    head,
                    ...tail.flatMap((child) => {
                      line++;

                      return [
                        "\n",
                        <span className="mr-4 inline-block w-4 select-none text-right italic text-[rgb(92,99,112)] last:hidden">
                          {line}
                        </span>,
                        child,
                      ];
                    }),
                  ];
                } else {
                  return child;
                }
              }),
            ],
          },
        }}
      </pre>
    </div>
  );
};

const mdxComponents: MDXRemoteProps["components"] = {
  a(props) {
    if (props.href.match(/^https?:\/\//)) {
      return (
        <ExternalLink
          {...props}
          className="text-brand-orange1 no-underline hover:text-brand-orange2"
        ></ExternalLink>
      );
    }

    return (
      <InternalLink
        {...(props as any)}
        className="text-brand-orange1 no-underline hover:text-brand-orange2"
      ></InternalLink>
    );
  },
  pre: (props: any) => {
    return <Pre {...props} />;
  },
  TLDR: (props: any) => {
    return (
      <div className="border-l-8 border-brand-orange1 bg-gray-200 p-4 text-left text-xl text-gray-500  dark:bg-gray-800 dark:text-gray-200">
        <span className="text-md rounded bg-brand-orange1 px-[10px] py-[2px] font-extrabold text-slate-100  dark:text-dark-800">
          TLDR
        </span>
        {props.children}
      </div>
    );
  },
  // blockquote(props) {
  //   return (
  //     <blockquote className="my-4 border-l-8 border-brand-orange1 bg-gray-200 p-4 text-left text-xl text-gray-500 dark:border-brand-orange2 dark:bg-gray-800 dark:text-gray-200">
  //       {props.children}
  //     </blockquote>
  //   );
  // },
};

interface Props {
  readonly prevPost?: Post;
  readonly nextPost?: Post;
  readonly relatedPosts: readonly Post[];
  readonly blog: Post;
}

export default function BlogPostPage(props: Props) {
  const author = getAuthors(props.blog.author?.split(",") ?? []);

  const { basePath } = useRouter();

  return (
    <>
      <NextSeo
        title={props.blog.title}
        openGraph={{
          title: props.blog.title,
          description: props.blog.description,
          url: `${SITE_URL}blog/${props.blog.slug}`,
          type: "article",
          article: {
            //
            // TODO: add expiration and modified dates
            // https://github.com/garmeeh/next-seo#article
            publishedTime: props.blog.date,
            //
            // TODO: author urls should be internal in future
            // currently we have external links to github profiles
            authors: [props.blog.author_url],
            tags: props.blog.tags.map((cat: string) => {
              return cat;
            }),
          },
          images: [
            {
              url: `${SITE_URL}${basePath}/images/blog/${props.blog.thumb}`,
            },
          ],
        }}
      />
      <div className="mx-auto max-w-6xl p-4 sm:p-6 lg:p-8">
        <div className="mb-16 max-w-5xl space-y-8">
          <div className="space-y-4">
            <p className="text-brand-900">Blog post</p>
            <h1 className="text-4xl">{props.blog.title}</h1>
            <div className="flex space-x-3 text-sm text-slate-500 dark:text-gray-400">
              <p>{props.blog.date}</p>
              <p>•</p>
              <p>{props.blog.readingTime}</p>
            </div>
            <div className="flex gap-3">
              {author.map((author, index) => {
                return (
                  <div className="mt-6 mb-8 mr-4 w-max lg:mb-0" key={index}>
                    <InternalLink
                      className={author.author_url ? "cursor-pointer" : ""}
                      href={author.author_url}
                    >
                      <div className="flex items-center gap-3">
                        {author.author_image_url && (
                          <div className="w-10">
                            <Image
                              src={author.author_image_url}
                              className="rounded-full border"
                              width="100%"
                              height="100%"
                              layout="responsive"
                            />
                          </div>
                        )}
                        <div className="flex flex-col">
                          <span className="mb-0 text-sm dark:text-gray-200">
                            {author.author}
                          </span>
                          <span className="mb-0 text-xs text-slate-500 dark:text-gray-400">
                            {author.position}
                          </span>
                        </div>
                      </div>
                    </InternalLink>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
        <div className="gap-6 lg:flex">
          {/* Content */}
          <div className="flex-1 overflow-hidden">
            {(props.blog.thumb ?? props.blog.cover) && (
              <div
                className="relative mb-8 aspect-[4/3] overflow-auto rounded"
                style={{
                  aspectRatio: props.blog.coverAspectRatio,
                }}
              >
                <Image
                  src={"/images/blog/" + (props.blog.cover ?? props.blog.thumb)}
                  layout="fill"
                  objectFit="cover"
                />
              </div>
            )}
            <article
              className={classnames(
                "prose dark:prose-invert",
                "max-w-none",
                "prose-headings:before:block",
                "prose-headings:before:-mt-36",
                "prose-headings:before:pt-36",
                "prose-headings:lg:before:-mt-20",
                "prose-headings:before:lg:pt-20"
              )}
            >
              <MDXRemote {...props.blog.content} components={mdxComponents} />
            </article>
            <div className="mt-16">
              <div className="text-sm dark:text-gray-400">
                Share this article
              </div>
              <div className="mt-4 flex items-center space-x-4">
                <ExternalLink
                  href={`https://twitter.com/share?text=${props.blog.title}&url=${SITE_URL}blog/${props.blog.slug}`}
                  className="text-slate-600 hover:text-slate-900 dark:text-gray-400 hover:dark:text-gray-200"
                >
                  <FontAwesomeIcon icon={faTwitter} className="text-xl" />
                </ExternalLink>

                <ExternalLink
                  href={`https://www.linkedin.com/shareArticle?url=${SITE_URL}blog/${props.blog.slug}&title=${props.blog.title}`}
                  className="text-slate-600 hover:text-slate-900 dark:text-gray-400 hover:dark:text-gray-200"
                >
                  <FontAwesomeIcon icon={faLinkedin} className="text-xl" />
                </ExternalLink>
              </div>
            </div>
            <div className="grid gap-8 py-8 lg:grid-cols-1">
              <div>
                {props.prevPost && (
                  <NextCard post={props.prevPost} label="Last post" />
                )}
              </div>
              <div>
                {props.nextPost && (
                  <NextCard
                    post={props.nextPost}
                    label="Next post"
                    className="text-right"
                  />
                )}
              </div>
            </div>
            <Socials />
          </div>
          {/* Sidebar */}
          <div className="flex-none space-y-8 lg:w-64">
            <div className="space-y-8 lg:sticky lg:top-20">
              <div className="hidden lg:block">
                <div className="space-y-8 py-8 lg:py-0">
                  <div className="space-x-2">
                    {props.blog.tags.map((tag: string) => {
                      return (
                        <InternalLink
                          key={tag}
                          className="flex-shrink-0 cursor-pointer rounded-full border border-green-400 bg-green-400/10 px-2 py-1 text-xs font-medium text-green-500 hover:bg-green-400/20 dark:text-green-400"
                          href={`/blog/tags/${tag}`}
                        >
                          {tag}
                        </InternalLink>
                      );
                    })}
                  </div>

                  <div className="mb-4 dark:text-gray-200">On this page</div>

                  <div className="prose prose-toc !mt-0 dark:prose-dark-toc">
                    <MDXRemote {...props.blog.toc} components={mdxComponents} />
                  </div>
                </div>
              </div>
              {props.relatedPosts.length > 0 ? (
                <div>
                  <div className="mb-4 dark:text-gray-200">
                    Related articles
                  </div>

                  <div className="flex flex-col gap-2 space-y-3">
                    {props.relatedPosts.map((post, index) => (
                      <InternalLink
                        href={`/blog/${post.url}`}
                        key={index}
                        className="flex gap-2 text-sm text-slate-500 hover:text-slate-900 dark:text-gray-300 hover:dark:text-gray-200"
                      >
                        <DocumentTextIcon className="mt-[2px] h-4 w-4 flex-shrink-0" />

                        <span>{post.title}</span>
                      </InternalLink>
                    ))}
                    <div className="mt-2">
                      <InternalLink
                        href={`/blog`}
                        className="cursor-pointer text-sm text-slate-500 hover:text-slate-900 dark:text-gray-300 hover:dark:text-gray-200"
                      >
                        View all posts
                      </InternalLink>
                    </div>
                  </div>
                </div>
              ) : null}
            </div>
          </div>
        </div>
      </div>
    </>
  );
}

interface NextCardProps {
  readonly post: Post;
  readonly label: string;
  readonly className?: string;
}

function NextCard({ post, label, className }: NextCardProps) {
  return (
    <InternalLink
      href={`/blog/${post.url}`}
      className={classnames(
        className,
        "block cursor-pointer rounded border border-slate-400 p-6 transition hover:bg-slate-200 dark:border-dark-500 dark:hover:bg-dark-600"
      )}
    >
      <div className="text-sm text-slate-500 dark:text-gray-400">{label}</div>

      <div className="mt-4 text-lg text-slate-800 dark:text-gray-200">
        {post.title}
      </div>
      <div className="text-xs text-slate-500 dark:text-gray-400">
        {post.date}
      </div>
    </InternalLink>
  );
}
