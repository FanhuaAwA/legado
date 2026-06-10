import type { ChapterItem } from "@/stores";

type ChapterVipLike = Pick<ChapterItem, "vip" | "isVip" | "price" | "currency">;

function normalizePriceLabel(price: unknown): string {
  if (typeof price === "string") {
    return price.trim();
  }
  if (typeof price === "number" || typeof price === "boolean" || typeof price === "bigint") {
    return price.toString().trim();
  }
  return "";
}

export function isVipChapter(chapter?: ChapterVipLike | null): boolean {
  return chapter?.vip === true || chapter?.isVip === true;
}

export function getChapterPriceLabel(chapter?: ChapterVipLike | null): string {
  if (!chapter || chapter.price === undefined || chapter.price === null) {
    return "";
  }
  const price = normalizePriceLabel(chapter.price);
  if (!price) {
    return "";
  }
  const currency = typeof chapter.currency === "string" ? chapter.currency.trim() : "";
  return currency ? `${price} ${currency}` : price;
}

export function isPurchaseResultOk(result: unknown): boolean {
  if (result === true) {
    return true;
  }
  if (result === false || result === null || result === undefined) {
    return false;
  }
  if (typeof result !== "object") {
    return true;
  }
  const record = result as Record<string, unknown>;
  if (typeof record.ok === "boolean") {
    return record.ok;
  }
  if (typeof record.success === "boolean") {
    return record.success;
  }
  if (typeof record.purchased === "boolean") {
    return record.purchased;
  }
  return true;
}

export function getPurchaseResultMessage(result: unknown): string {
  if (result && typeof result === "object") {
    const message = (result as Record<string, unknown>).message;
    if (typeof message === "string" && message.trim()) {
      return message.trim();
    }
  }
  return "";
}
