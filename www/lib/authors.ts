export interface Author {
  readonly author_id: string;
  readonly author: string;
  readonly position: string;
  readonly author_url: string;
  readonly author_image_url: string;
}

export function getAuthors(ids: readonly string[]): readonly Author[] {
  return ids.flatMap((id) => {
    const author = authors.find((author) => author.author_id === id);

    if (author == null) return [];

    return [author];
  });
}

const authors: readonly Author[] = [
  {
    author_id: "christoshadjiaslanis",
    author: "Christos Hadjiaslanis",
    position: "Founder",
    author_url: "https://github.com/christoshadjiaslanis",
    author_image_url: "https://github.com/christoshadjiaslanis.png",
  },
  {
    author_id: "brokad",
    author: "Damien Broka",
    position: "Founder",
    author_url: "https://github.com/brokad",
    author_image_url: "https://github.com/brokad.png",
  },
  {
    author_id: "terrencewaters",
    author: "Terrence Waters",
    position: "Software Engineer",
    author_url: "",
    author_image_url: "",
  }
];

export default authors;
