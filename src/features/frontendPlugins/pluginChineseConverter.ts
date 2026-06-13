import * as OpenCC from "opencc-js";
import type { ChineseConvertMode } from "./pluginTypes";

const chineseConverterCache = new Map<ChineseConvertMode, (text: string) => string>();

export function getChineseConverter(mode: ChineseConvertMode): (text: string) => string {
  const cached = chineseConverterCache.get(mode);
  if (cached) {
    return cached;
  }
  const converter = (() => {
    switch (mode) {
      case "s2t":
        return OpenCC.Converter({ from: "cn", to: "t" });
      case "s2tw":
        return OpenCC.Converter({ from: "cn", to: "tw" });
      case "s2hk":
        return OpenCC.Converter({ from: "cn", to: "hk" });
      case "t2s":
        return OpenCC.Converter({ from: "t", to: "cn" });
      case "tw2s":
        return OpenCC.Converter({ from: "tw", to: "cn" });
      case "hk2s":
        return OpenCC.Converter({ from: "hk", to: "cn" });
      default:
        return (text: string) => text;
    }
  })();
  chineseConverterCache.set(mode, converter);
  return converter;
}
