import type { AggregatedBook, BookItem, TaggedBookItem } from "@/types";

function normalizedText(value: string): string {
  return value.toLowerCase().replace(/\s+/g, "");
}

function bigrams(value: string): Set<string> {
  const normalized = normalizedText(value);
  const set = new Set<string>();
  for (let index = 0; index < normalized.length - 1; index += 1) {
    set.add(normalized.substring(index, index + 2));
  }
  return set;
}

export function diceSimilarity(a: string, b: string): number {
  if (!a || !b) {
    return 0;
  }
  const normalizedA = normalizedText(a);
  const normalizedB = normalizedText(b);
  if (normalizedA === normalizedB) {
    return 1;
  }
  if (normalizedA.length < 2 || normalizedB.length < 2) {
    return normalizedA.includes(normalizedB) || normalizedB.includes(normalizedA) ? 0.8 : 0;
  }

  const aBigrams = bigrams(a);
  const bBigrams = bigrams(b);
  let intersection = 0;
  aBigrams.forEach((gram) => {
    if (bBigrams.has(gram)) {
      intersection += 1;
    }
  });
  return (2 * intersection) / (aBigrams.size + bBigrams.size);
}

export function isSameBook(a: BookItem, b: BookItem): boolean {
  const nameA = normalizedText(a.name);
  const nameB = normalizedText(b.name);
  if (nameA === nameB) {
    return true;
  }

  const similarity = diceSimilarity(a.name, b.name);
  if (similarity >= 0.85) {
    return true;
  }
  return similarity >= 0.7 && Boolean(a.author && b.author && a.author.trim() === b.author.trim());
}

export function appendTaggedResultsToGroups(
  groups: AggregatedBook[],
  items: readonly TaggedBookItem[],
  keyword: string,
): void {
  const normalizedKeyword = keyword.trim();
  for (const item of items) {
    const similarity = diceSimilarity(item.book.name, normalizedKeyword);
    let matched = false;
    for (const group of groups) {
      if (!isSameBook(group.primary.book, item.book)) {
        continue;
      }
      group.sources.push(item);
      if (!group.primary.book.coverUrl && item.book.coverUrl) {
        group.primary = item;
      }
      if (similarity > group.similarity) {
        group.similarity = similarity;
      }
      matched = true;
      break;
    }

    if (!matched) {
      groups.push({
        primary: item,
        sources: [item],
        similarity,
      });
    }
  }
}

export function sortAggregatedGroups(groups: AggregatedBook[]): void {
  groups.sort((a, b) => b.similarity - a.similarity);
}

export function aggregateTaggedResults(
  results: readonly TaggedBookItem[],
  keyword: string,
): AggregatedBook[] {
  const trimmedKeyword = keyword.trim();
  if (!trimmedKeyword || !results.length) {
    return [];
  }
  const groups: AggregatedBook[] = [];
  appendTaggedResultsToGroups(groups, results, trimmedKeyword);
  sortAggregatedGroups(groups);
  return groups;
}
