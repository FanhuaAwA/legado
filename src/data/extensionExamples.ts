import { parseUserScriptMeta, type ExtensionMeta } from "../composables/useExtension";

export interface ExampleScript {
  id: string;
  source: string;
  meta: Partial<ExtensionMeta>;
}

function make(id: string, source: string): ExampleScript {
  return { id, source, meta: parseUserScriptMeta(source) };
}

interface ExampleScriptLoader {
  id: string;
  load: () => Promise<string>;
}

const EXAMPLE_SCRIPT_LOADERS: ExampleScriptLoader[] = [
  {
    id: "filter",
    load: () => import("./pluginExamples/reader-ad-cleaner.js?raw").then((mod) => mod.default),
  },
  {
    id: "replace",
    load: () => import("./pluginExamples/reader-text-replacer.js?raw").then((mod) => mod.default),
  },
  {
    id: "selection-tools",
    load: () => import("./pluginExamples/reader-selection-tools.js?raw").then((mod) => mod.default),
  },
  {
    id: "timer",
    load: () => import("./pluginExamples/reader-timer.js?raw").then((mod) => mod.default),
  },
  {
    id: "progress",
    load: () => import("./pluginExamples/reader-progress-badge.js?raw").then((mod) => mod.default),
  },
  {
    id: "top-progress-bar",
    load: () =>
      import("./pluginExamples/reader-top-progress-bar.js?raw").then((mod) => mod.default),
  },
  {
    id: "split",
    load: () =>
      import("./pluginExamples/reader-paragraph-splitter.js?raw").then((mod) => mod.default),
  },
  {
    id: "custom-theme",
    load: () =>
      import("./pluginExamples/reader-custom-color-theme.js?raw").then((mod) => mod.default),
  },
  {
    id: "custom-background",
    load: () =>
      import("./pluginExamples/reader-custom-backgrounds.js?raw").then((mod) => mod.default),
  },
  {
    id: "uploaded-background-images",
    load: () =>
      import("./pluginExamples/reader-uploaded-background-images.js?raw").then(
        (mod) => mod.default,
      ),
  },
  {
    id: "disguise-skins",
    load: () => import("./pluginExamples/reader-disguise-skins.js?raw").then((mod) => mod.default),
  },
  {
    id: "blue-theme",
    load: () =>
      import("./pluginExamples/reader-theme-blue-ocean.js?raw").then((mod) => mod.default),
  },
  {
    id: "paper-pack",
    load: () =>
      import("./pluginExamples/reader-background-paper-pack.js?raw").then((mod) => mod.default),
  },
  {
    id: "night-pack",
    load: () =>
      import("./pluginExamples/reader-background-night-pack.js?raw").then((mod) => mod.default),
  },
  {
    id: "chinese-converter",
    load: () =>
      import("./pluginExamples/reader-chinese-converter.js?raw").then((mod) => mod.default),
  },
  {
    id: "bookshelf-openlibrary",
    load: () =>
      import("./pluginExamples/bookshelf-openlibrary-enricher.js?raw").then((mod) => mod.default),
  },
  {
    id: "bookshelf-cover-studio",
    load: () => import("./pluginExamples/bookshelf-cover-studio.js?raw").then((mod) => mod.default),
  },
  {
    id: "community-cover-pack",
    load: () =>
      import("./pluginExamples/bookshelf-community-cover-pack.js?raw").then((mod) => mod.default),
  },
  {
    id: "word-counter",
    load: () => import("./pluginExamples/reader-word-counter.js?raw").then((mod) => mod.default),
  },
  {
    id: "auto-theme",
    load: () => import("./pluginExamples/reader-auto-theme.js?raw").then((mod) => mod.default),
  },
  {
    id: "export-notes",
    load: () => import("./pluginExamples/bookshelf-export-notes.js?raw").then((mod) => mod.default),
  },
  {
    id: "custom-inject",
    load: () => import("./pluginExamples/reader-custom-inject.js?raw").then((mod) => mod.default),
  },
  {
    id: "tts-edge-read-aloud",
    load: () => import("./pluginExamples/tts-edge-read-aloud.js?raw").then((mod) => mod.default),
  },
];

let exampleScriptsPromise: Promise<ExampleScript[]> | null = null;

export function loadExampleScripts(): Promise<ExampleScript[]> {
  exampleScriptsPromise ??= Promise.all(
    EXAMPLE_SCRIPT_LOADERS.map(async (item) => make(item.id, await item.load())),
  );
  return exampleScriptsPromise;
}
