import { parseUserScriptMeta, type ExtensionMeta } from "@/composables/useExtension";
import mimoTtsSource from "./pluginExamples/tts-xiaomi-mimo-v25.js?raw";

export interface BuiltinPluginDefinition {
  id: string;
  source: string;
  meta: ExtensionMeta;
}

function makeBuiltinPlugin(id: string, fileName: string, source: string): BuiltinPluginDefinition {
  const parsed = parseUserScriptMeta(source);
  return {
    id,
    source,
    meta: {
      fileName,
      name: parsed.name ?? id,
      namespace: parsed.namespace ?? id,
      version: parsed.version ?? "0.0.0",
      description: parsed.description ?? "",
      author: parsed.author ?? "",
      matchPatterns: parsed.matchPatterns ?? ["*"],
      grants: parsed.grants ?? [],
      runAt: parsed.runAt ?? "document-idle",
      category: parsed.category ?? "其他",
      enabled: parsed.enabled ?? true,
      fileSize: source.length,
      modifiedAt: 0,
    },
  };
}

export const BUILTIN_FRONTEND_PLUGINS: BuiltinPluginDefinition[] = [
  makeBuiltinPlugin("tts-xiaomi-mimo-v25", "builtin-tts-xiaomi-mimo-v25.js", mimoTtsSource),
];
