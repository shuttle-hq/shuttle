import { NextSeo } from "next-seo";
import { getAlltags, getSortedPosts, Post } from "../../../lib/posts";
import BlogListItem from "../../../components/blog/BlogListItem";
import { ParsedUrlQuery } from "querystring";
import { GetStaticPropsContext, GetStaticPropsResult } from "next";
import InternalLink from "../../../components/InternalLink";

interface Params extends ParsedUrlQuery {
  readonly tag: string;
}

export async function getStaticProps({
  params,
}: GetStaticPropsContext<Params>): Promise<GetStaticPropsResult<Props>> {
  const posts = getSortedPosts(0, [params.tag]);

  if (process.env.NODE_ENV === "production") {
    return {
      notFound: true,
    };
  }

  return {
    props: {
      tag: params.tag,
      blogs: posts,
    },
  };
}

export async function getStaticPaths() {
  const tags = getAlltags();
  return {
    paths: tags.map((tag) => ({ params: { tag } })),
    fallback: false,
  };
}

interface Props {
  readonly tag: string;
  readonly blogs: readonly Post[];
}

export default function TagBlogsPage(props: Props) {
  const { blogs, tag } = props;
  return (
    <>
      <NextSeo
        title={`Blog | ${tag}`}
        description="Latest news from the shuttle team."
      />

      <div className="container mx-auto px-8 py-16 sm:px-16 xl:px-20">
        <div className="flex space-x-1">
          <p className="cursor-pointer">
            <InternalLink href="/blog">Blog</InternalLink>
          </p>
          <p>/</p>
          <p>{`${tag}`}</p>
        </div>
        <ol className="grid grid-cols-12 gap-8 py-16 lg:gap-16">
          {blogs.map((blog, idx) => (
            <div
              className="col-span-12 mb-16 md:col-span-12 lg:col-span-6 xl:col-span-4"
              key={idx}
            >
              <BlogListItem post={blog} />
            </div>
          ))}
        </ol>
      </div>
    </>
  );
}
