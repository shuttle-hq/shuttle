export function generateReadingTime(text: string): string {
  const wordsPerMinute = 200;
  const noOfWords = text.split(/\s/g).length;
  const minutes = noOfWords / wordsPerMinute;
  const readTime = Math.ceil(minutes);
  return `${readTime} minute read`;
}

export function capitalize(word: string): string {
  return word[0].toUpperCase() + word.substring(1).toLowerCase();
}
