import { invokeWithTimeout } from "./useInvoke";
import { useCapabilities } from "./useCapabilities";

export interface BrowserCreateOptions {
  visible?: boolean;
  userAgent?: string;
  timeoutSecs?: number;
  timeout?: number;
  timeoutMs?: number;
  width?: number;
  height?: number;
}

export interface BrowserNavigateOptions {
  waitUntil?: "load" | "domcontentloaded" | "networkidle";
  waitFor?: "load" | "domcontentloaded" | "networkidle";
  timeoutSecs?: number;
  timeout?: number;
  timeoutMs?: number;
}

export interface BrowserEvalOptions {
  timeoutSecs?: number;
  timeout?: number;
  timeoutMs?: number;
}

export interface BrowserRunOptions extends BrowserCreateOptions, BrowserNavigateOptions {}

export interface BrowserCookie {
  name: string;
  value: string;
  domain?: string;
  path?: string;
  expires?: number;
  httpOnly?: boolean;
  secure?: boolean;
  sameSite?: string;
}

const DEFAULT_TIMEOUT = 60_000;
const capabilities = useCapabilities();

async function requireBrowserProbeCapability(): Promise<void> {
  await capabilities.requireCapability("browserProbe");
}

function browserTimeoutMs(options: { timeoutSecs?: number; timeout?: number; timeoutMs?: number }) {
  return (
    (options.timeoutSecs ?? options.timeout ?? (options.timeoutMs ?? DEFAULT_TIMEOUT) / 1000) * 1000
  );
}

export async function browserProbeCreate(options: BrowserCreateOptions = {}) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<string>("browser_probe_create", { options }, DEFAULT_TIMEOUT);
}

export async function browserProbeNavigate(
  sessionId: string,
  url: string,
  options: BrowserNavigateOptions = {},
) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<void>(
    "browser_probe_navigate",
    { sessionId, url, options },
    browserTimeoutMs(options) + 5000,
  );
}

export async function browserProbeEval<T = unknown>(
  sessionId: string,
  code: string,
  options: BrowserEvalOptions = {},
) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<T>(
    "browser_probe_eval",
    { sessionId, code, options },
    browserTimeoutMs(options) + 5000,
  );
}

export async function browserProbeRun<T = unknown>(
  url: string,
  code: string,
  options: BrowserRunOptions = {},
) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<T>(
    "browser_probe_run",
    { url, code, options },
    browserTimeoutMs(options) + 10_000,
  );
}

export async function browserProbeGetCookies(url?: string) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<BrowserCookie[]>(
    "browser_probe_get_cookies",
    { url: url ?? null },
    DEFAULT_TIMEOUT,
  );
}

export async function browserProbeSetCookie(url: string, cookie: BrowserCookie) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<void>("browser_probe_set_cookie", { url, cookie }, DEFAULT_TIMEOUT);
}

export async function browserProbeSetUserAgent(userAgent: string) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<void>("browser_probe_set_user_agent", { userAgent }, DEFAULT_TIMEOUT);
}

export async function browserProbeClearData() {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<void>("browser_probe_clear_data", {}, DEFAULT_TIMEOUT);
}

export async function browserProbeShow(sessionId: string) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<void>("browser_probe_show", { sessionId }, DEFAULT_TIMEOUT);
}

export async function browserProbeHide(sessionId: string) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<void>("browser_probe_hide", { sessionId }, DEFAULT_TIMEOUT);
}

export async function browserProbeClose(sessionId: string) {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<void>("browser_probe_close", { sessionId }, DEFAULT_TIMEOUT);
}

export async function browserProbeCloseAll() {
  await requireBrowserProbeCapability();
  return invokeWithTimeout<void>("browser_probe_close_all", {}, DEFAULT_TIMEOUT);
}
