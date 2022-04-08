import authors, { getAuthors } from "../../lib/authors";
import Image from "next/image";
import React from "react";
import { Post } from "../../lib/posts";
import InternalLink from "../InternalLink";

interface Props {
  readonly post: Post;
}

export default function BlogListItem({ post }: Props): JSX.Element {
  const author = getAuthors(post.author?.split(",") ?? []);

  return (
    <div>
      <InternalLink href={`/blog/${post.url}`}>
        <div className="group inline-block min-w-full">
          <div className="flex flex-col space-y-6">
            <div className="flex flex-col space-y-3">
              <div
                className={`relative mb-4 aspect-[4/3] w-full overflow-auto rounded shadow-md`}
              >
                <Image
                  layout="fill"
                  src={
                    !post.thumb
                      ? `/images/blog/blog-placeholder.png`
                      : `/images/blog/${post.thumb}`
                  }
                  objectFit="cover"
                />
              </div>

              <h3 className="max-w-sm text-xl text-gray-200">{post.title}</h3>
              {post.date && (
                <p className="text-xs text-gray-300">{post.date}</p>
              )}
              <p className="max-w-sm text-base text-gray-300">
                {post.description}
              </p>
            </div>
            <div className="flex items-center -space-x-2">
              {author.map((author, index) => {
                return (
                  <div
                    className="z-0 w-10 overflow-hidden rounded-full border-2 border-gray-500"
                    key={index}
                  >
                    {author.author_image_url && (
                      <Image
                        src={author.author_image_url}
                        width="100%"
                        height="100%"
                        layout="responsive"
                      />
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </InternalLink>
    </div>
  );
}
