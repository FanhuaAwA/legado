#!/usr/bin/env node
import { webcrypto } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import vm from "node:vm";

const ROOT = process.cwd();
const EXAMPLE_DIR = path.join(ROOT, "src", "data", "pluginExamples");
const CAPABILITY_KEYS = [
  "hooks",
  "slots",
  "themes",
  "backgrounds",
  "skins",
  "bookshelfActions",
  "readerContextActions",
  "coverGenerators",
  "ttsEngines",
];

function createElementStub(tagName = "div") {
  const element = {
    tagName: String(tagName).toUpperCase(),
    style: {},
    dataset: {},
    children: [],
    className: "",
    innerHTML: "",
    textContent: "",
    value: "",
    checked: false,
    classList: {
      add() {},
      remove() {},
      toggle() {
        return false;
      },
      contains() {
        return false;
      },
    },
    appendChild(child) {
      this.children.push(child);
      return child;
    },
    removeChild(child) {
      this.children = this.children.filter((item) => item !== child);
      return child;
    },
    setAttribute(name, value) {
      this[name] = value;
    },
    getAttribute(name) {
      return this[name] ?? null;
    },
    addEventListener() {},
    removeEventListener() {},
    querySelector() {
      return null;
    },
    querySelectorAll() {
      return [];
    },
    remove() {},
  };
  return element;
}

function createDocumentStub() {
  const head = createElementStub("head");
  const body = createElementStub("body");
  return {
    head,
    body,
    documentElement: createElementStub("html"),
    createElement: createElementStub,
    createTextNode(text) {
      return { nodeType: 3, textContent: String(text) };
    },
    getElementById() {
      return null;
    },
    querySelector() {
      return null;
    },
    querySelectorAll() {
      return [];
    },
    addEventListener() {},
    removeEventListener() {},
  };
}

function createWebSocketStub() {
  return class WebSocketStub {
    static OPEN = 1;

    constructor(url) {
      this.url = url;
      this.readyState = WebSocketStub.OPEN;
      setTimeout(() => this.onopen?.({ type: "open" }), 0);
    }

    send() {}

    close() {
      this.readyState = 3;
      this.onclose?.({ type: "close" });
    }

    addEventListener(type, listener) {
      this[`on${type}`] = listener;
    }

    removeEventListener(type) {
      this[`on${type}`] = null;
    }
  };
}

function createPluginApi() {
  const settings = new Map();
  const storage = new Map();
  const session = {
    book: {
      id: "demo-book",
      name: "Demo Book",
      author: "Demo Author",
      kind: "novel",
    },
    chapterIndex: 0,
    chapterTitle: "Chapter 1",
    progress: 0.5,
    readSeconds: 60,
  };

  return {
    settings: {
      get(key, fallback) {
        return settings.has(key) ? settings.get(key) : fallback;
      },
      getAll() {
        return Object.fromEntries(settings.entries());
      },
      async set(key, value) {
        settings.set(key, value);
      },
      async remove(key) {
        settings.delete(key);
      },
      async reset() {
        settings.clear();
      },
    },
    storage: {
      read(key, fallback = "") {
        return storage.has(key) ? storage.get(key) : fallback;
      },
      write(key, value) {
        storage.set(key, value);
      },
      readJson(key, fallback) {
        return storage.has(key) ? storage.get(key) : fallback;
      },
      writeJson(key, value) {
        storage.set(key, value);
      },
      remove(key) {
        storage.delete(key);
      },
    },
    reader: {
      getSession() {
        return session;
      },
      onSessionChange(listener) {
        if (typeof listener === "function") {
          listener(session);
        }
        return () => {};
      },
      refreshAppearance: async () => {},
      remountSlots: async () => {},
    },
    ui: {
      toast: async () => {},
      prompt: async (config = {}) => config.initialValues ?? null,
      getAppTheme: () => "auto",
      setAppTheme: async () => {},
    },
    http: {
      get: async () => "{}",
      post: async () => "{}",
      request: async () => ({ status: 200, headers: {}, body: "{}" }),
    },
    bookshelf: {
      getBook: async () => session.book,
      patchBook: async (_id, patch) => ({ ...session.book, ...patch }),
    },
    text: {
      convertChinese: (text) => text,
    },
    assets: {
      resolve: (value) => value,
    },
    log: () => {},
    registerCleanup: () => {},
  };
}

function createContext(registrations) {
  const document = createDocumentStub();
  const window = {
    document,
    navigator: {
      clipboard: {
        writeText: async () => {},
      },
    },
    addEventListener() {},
    removeEventListener() {},
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
  };

  return vm.createContext({
    legado: {
      registerPlugin(registration) {
        registrations.push(registration);
      },
    },
    console,
    window,
    document,
    navigator: window.navigator,
    crypto: webcrypto,
    TextEncoder,
    TextDecoder,
    Blob,
    URL: {
      createObjectURL: () => "blob:plugin-example",
      revokeObjectURL: () => {},
    },
    Audio: class AudioStub {
      play() {
        return Promise.resolve();
      }
      pause() {}
    },
    WebSocket: createWebSocketStub(),
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
  });
}

function hasUsefulCapability(capabilities) {
  return CAPABILITY_KEYS.some((key) => {
    const value = capabilities[key];
    if (!value) {
      return false;
    }
    if (Array.isArray(value)) {
      return value.length > 0;
    }
    if (typeof value === "function") {
      return true;
    }
    if (typeof value === "object") {
      return Object.keys(value).length > 0;
    }
    return false;
  });
}

async function checkFile(fileName) {
  const source = await readFile(path.join(EXAMPLE_DIR, fileName), "utf8");
  const registrations = [];
  const context = createContext(registrations);
  vm.runInContext(source, context, { filename: fileName, timeout: 2_000 });

  if (registrations.length !== 1) {
    throw new Error(`expected 1 registration, got ${registrations.length}`);
  }
  const registration = registrations[0];
  if (!registration || typeof registration !== "object") {
    throw new Error("registration is not an object");
  }
  if (typeof registration.id !== "string" || registration.id.trim() === "") {
    throw new Error("registration.id is missing");
  }

  const setupResult =
    typeof registration.setup === "function"
      ? await registration.setup(createPluginApi())
      : undefined;
  const capabilities = { ...registration, ...setupResult };
  if (!hasUsefulCapability(capabilities)) {
    throw new Error("no useful runtime capability was registered");
  }

  return { fileName, id: registration.id };
}

const files = (await readdir(EXAMPLE_DIR)).filter((fileName) => fileName.endsWith(".js")).sort();
const results = [];
const failures = [];

for (const fileName of files) {
  try {
    results.push(await checkFile(fileName));
  } catch (error) {
    failures.push({
      fileName,
      message: error instanceof Error ? error.message : String(error),
    });
  }
}

if (failures.length > 0) {
  console.error("Plugin example check failed:");
  for (const failure of failures) {
    console.error(`- ${failure.fileName}: ${failure.message}`);
  }
  process.exit(1);
}

console.log(`Plugin example check passed: ${results.length}/${files.length}`);
