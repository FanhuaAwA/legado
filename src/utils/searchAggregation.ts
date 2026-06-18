import type { AggregatedBook, BookItem, TaggedBookItem } from "@/types";

function normalizedText(value: string): string {
  return value.toLowerCase().replace(/\s+/g, "");
}

function bigramsFromNormalized(normalized: string): Set<string> {
  const set = new Set<string>();
  for (let index = 0; index < normalized.length - 1; index += 1) {
    set.add(normalized.substring(index, index + 2));
  }
  return set;
}

function bigrams(value: string): Set<string> {
  return bigramsFromNormalized(normalizedText(value));
}

interface AggregationIndex {
  exactName: Map<string, AggregatedBook>;
  bigramGroups: Map<string, Set<AggregatedBook>>;
  shortNameGroups: Set<AggregatedBook>;
}

const aggregationIndexes = new WeakMap<AggregatedBook[], AggregationIndex>();

function createAggregationIndex(): AggregationIndex {
  return {
    exactName: new Map(),
    bigramGroups: new Map(),
    shortNameGroups: new Set(),
  };
}

function addGroupToIndex(index: AggregationIndex, group: AggregatedBook): void {
  const name = normalizedText(group.primary.book.name);
  if (name && !index.exactName.has(name)) {
    index.exactName.set(name, group);
  }

  const grams = bigramsFromNormalized(name);
  if (grams.size === 0) {
    index.shortNameGroups.add(group);
    return;
  }

  for (const gram of grams) {
    let groups = index.bigramGroups.get(gram);
    if (!groups) {
      groups = new Set();
      index.bigramGroups.set(gram, groups);
    }
    groups.add(group);
  }
}

function getAggregationIndex(groups: AggregatedBook[]): AggregationIndex {
  let index = aggregationIndexes.get(groups);
  if (!index) {
    index = createAggregationIndex();
    for (const group of groups) {
      addGroupToIndex(index, group);
    }
    aggregationIndexes.set(groups, index);
  }
  return index;
}

function findMatchingGroup(
  groups: AggregatedBook[],
  index: AggregationIndex,
  item: TaggedBookItem,
): AggregatedBook | undefined {
  const name = normalizedText(item.book.name);
  const exactMatch = index.exactName.get(name);
  if (exactMatch && isSameBook(exactMatch.primary.book, item.book)) {
    return exactMatch;
  }

  const grams = bigramsFromNormalized(name);
  if (grams.size === 0) {
    return groups.find((group) => isSameBook(group.primary.book, item.book));
  }

  const candidates = new Set(index.shortNameGroups);
  for (const gram of grams) {
    const gramGroups = index.bigramGroups.get(gram);
    if (gramGroups) {
      gramGroups.forEach((group) => candidates.add(group));
    }
  }
  for (const group of candidates) {
    if (isSameBook(group.primary.book, item.book)) {
      return group;
    }
  }
  return undefined;
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
  const index = getAggregationIndex(groups);
  for (const item of items) {
    const similarity = diceSimilarity(item.book.name, normalizedKeyword);
    const group = findMatchingGroup(groups, index, item);
    if (group) {
      group.sources.push(item);
      if (!group.primary.book.coverUrl && item.book.coverUrl) {
        group.primary = item;
        addGroupToIndex(index, group);
      }
      if (similarity > group.similarity) {
        group.similarity = similarity;
      }
    } else {
      const nextGroup = {
        primary: item,
        sources: [item],
        similarity,
      };
      groups.push(nextGroup);
      addGroupToIndex(index, nextGroup);
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
