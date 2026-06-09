export type AppUpdateChannel = "stable" | "development";

export type AppUpdatePlatform =
  | "windows"
  | "macos"
  | "linux"
  | "android"
  | "ios"
  | "harmony"
  | "unknown";

export interface AppUpdateAsset {
  name: string;
  url: string;
  size: number;
  digest: string;
}

export interface AppUpdateCheckResult {
  channel: AppUpdateChannel;
  platform: AppUpdatePlatform;
  platformLabel: string;
  currentVersion: string;
  latestVersion: string;
  latestDisplayVersion: string;
  releaseName: string;
  releaseTag: string;
  releaseUrl: string;
  releasePublishedAt: string;
  asset: AppUpdateAsset | null;
  hasUpdate: boolean;
  scannedReleaseCount: number;
  unavailableReason: string;
}

interface CheckAppUpdateOptions {
  channel: AppUpdateChannel;
  currentVersion: string;
  platform: string;
}

interface GitHubReleaseAsset {
  name: string;
  size?: number;
  digest?: string | null;
  browser_download_url: string;
}

interface GitHubRelease {
  tag_name: string;
  name: string | null;
  html_url: string;
  draft: boolean;
  prerelease: boolean;
  published_at: string | null;
  body: string | null;
  assets: GitHubReleaseAsset[];
}

interface ParsedReleaseVersion {
  version: string;
  displayVersion: string;
}

interface ChannelConfig {
  repo: string;
  releasePageUrl: string;
  perPage: number;
  maxPages: number;
}

const CHANNEL_CONFIGS: Record<AppUpdateChannel, ChannelConfig> = {
  stable: {
    repo: "LegadoTeam/Legado-Tauri",
    releasePageUrl: "https://github.com/LegadoTeam/Legado-Tauri/releases",
    perPage: 20,
    maxPages: 3,
  },
  development: {
    repo: "LegadoTeam/Legado-Tauri-Release",
    releasePageUrl: "https://github.com/LegadoTeam/Legado-Tauri-Release/releases",
    perPage: 20,
    maxPages: 10,
  },
};

const FETCH_TIMEOUT_MS = 20_000;

export function isAppUpdateChannel(value: string): value is AppUpdateChannel {
  return value === "stable" || value === "development";
}

export function getAppUpdateChannelLabel(channel: AppUpdateChannel): string {
  return channel === "development" ? "开发" : "正式";
}

export function getAppUpdateReleasePageUrl(channel: AppUpdateChannel): string {
  return CHANNEL_CONFIGS[channel].releasePageUrl;
}

export function normalizeCoreVersion(value: string): string {
  const match = String(value ?? "").match(/v?(\d+)\.(\d+)\.(\d+)/i);
  if (!match) {
    return "";
  }
  return `${match[1]}.${match[2]}.${match[3]}`;
}

export function compareCoreVersions(left: string, right: string): 1 | -1 | 0 | null {
  const leftCore = normalizeCoreVersion(left);
  const rightCore = normalizeCoreVersion(right);
  if (!leftCore || !rightCore) {
    return null;
  }

  const leftParts = leftCore.split(".").map((part) => Number.parseInt(part, 10));
  const rightParts = rightCore.split(".").map((part) => Number.parseInt(part, 10));
  for (let index = 0; index < 3; index += 1) {
    const leftPart = leftParts[index] ?? 0;
    const rightPart = rightParts[index] ?? 0;
    if (leftPart > rightPart) {
      return 1;
    }
    if (leftPart < rightPart) {
      return -1;
    }
  }
  return 0;
}

export function resolveAppUpdatePlatform(rawPlatform: string): {
  platform: AppUpdatePlatform;
  label: string;
} {
  const normalized = rawPlatform.trim().toLowerCase();
  if (normalized.includes("windows") || normalized === "win32") {
    return { platform: "windows", label: "Windows" };
  }
  if (normalized.includes("mac") || normalized === "darwin") {
    return { platform: "macos", label: "macOS" };
  }
  if (normalized.includes("android")) {
    return { platform: "android", label: "Android" };
  }
  if (normalized.includes("ios") || normalized.includes("iphone") || normalized.includes("ipad")) {
    return { platform: "ios", label: "iOS" };
  }
  if (normalized.includes("harmony") || normalized.includes("openharmony")) {
    return { platform: "harmony", label: "HarmonyOS" };
  }
  if (normalized.includes("linux")) {
    return { platform: "linux", label: "Linux" };
  }
  return { platform: "unknown", label: rawPlatform || "未知平台" };
}

export function formatAppUpdateAssetSize(size: number): string {
  if (!Number.isFinite(size) || size <= 0) {
    return "未知大小";
  }

  const units = ["B", "KB", "MB", "GB"];
  let value = size;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  const fixed = value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1);
  return `${fixed} ${units[unitIndex]}`;
}

export async function checkAppUpdate(
  options: CheckAppUpdateOptions,
): Promise<AppUpdateCheckResult> {
  const channelConfig = CHANNEL_CONFIGS[options.channel];
  const platformInfo = resolveAppUpdatePlatform(options.platform);
  const currentVersion = normalizeCoreVersion(options.currentVersion);

  if (!currentVersion) {
    throw new Error(`无法识别当前版本号: ${options.currentVersion || "空"}`);
  }

  if (platformInfo.platform === "unknown") {
    throw new Error(`无法识别当前平台: ${platformInfo.label}`);
  }

  let scannedReleaseCount = 0;
  let latestParsed: GitHubRelease | null = null;
  let latestParsedVersion: ParsedReleaseVersion | null = null;

  for (let page = 1; page <= channelConfig.maxPages; page += 1) {
    const releases = await fetchReleases(channelConfig, page);
    if (releases.length === 0) {
      break;
    }

    for (const release of releases) {
      if (release.draft) {
        continue;
      }

      const parsedVersion = parseReleaseVersion(release, options.channel);
      if (!parsedVersion) {
        continue;
      }

      scannedReleaseCount += 1;
      latestParsed ??= release;
      latestParsedVersion ??= parsedVersion;

      const asset = pickPlatformAsset(release.assets, platformInfo.platform, options.channel);
      if (!asset) {
        continue;
      }

      return buildResult({
        channel: options.channel,
        platformInfo,
        currentVersion,
        release,
        parsedVersion,
        asset,
        scannedReleaseCount,
        unavailableReason: "",
      });
    }
  }

  if (latestParsed && latestParsedVersion) {
    return buildResult({
      channel: options.channel,
      platformInfo,
      currentVersion,
      release: latestParsed,
      parsedVersion: latestParsedVersion,
      asset: null,
      scannedReleaseCount,
      unavailableReason: `最近 ${scannedReleaseCount} 个可解析发布中没有 ${platformInfo.label} 安装包`,
    });
  }

  return {
    channel: options.channel,
    platform: platformInfo.platform,
    platformLabel: platformInfo.label,
    currentVersion,
    latestVersion: "",
    latestDisplayVersion: "",
    releaseName: "",
    releaseTag: "",
    releaseUrl: channelConfig.releasePageUrl,
    releasePublishedAt: "",
    asset: null,
    hasUpdate: false,
    scannedReleaseCount,
    unavailableReason: "未找到可解析的发布版本",
  };
}

async function fetchReleases(config: ChannelConfig, page: number): Promise<GitHubRelease[]> {
  const url = new URL(`https://api.github.com/repos/${config.repo}/releases`);
  url.searchParams.set("per_page", String(config.perPage));
  url.searchParams.set("page", String(page));

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
  try {
    const response = await fetch(url.toString(), {
      headers: {
        Accept: "application/vnd.github+json",
      },
      signal: controller.signal,
    });

    if (!response.ok) {
      const remaining = response.headers.get("x-ratelimit-remaining");
      if (response.status === 403 && remaining === "0") {
        throw new Error("GitHub API 访问频率已用尽，请稍后再试");
      }
      throw new Error(`GitHub releases 请求失败: HTTP ${response.status}`);
    }

    return (await response.json()) as GitHubRelease[];
  } finally {
    clearTimeout(timer);
  }
}

function parseReleaseVersion(
  release: GitHubRelease,
  channel: AppUpdateChannel,
): ParsedReleaseVersion | null {
  return channel === "development"
    ? parseDevelopmentReleaseVersion(release)
    : parseStableReleaseVersion(release);
}

function parseStableReleaseVersion(release: GitHubRelease): ParsedReleaseVersion | null {
  const source = `${release.tag_name} ${release.name ?? ""}`;
  const betaMatch = source.match(/\bBeta[-_]?v?(\d+\.\d+\.\d+)(?=[-+\s]|$)/i);
  const fallbackMatch = source.match(/\bv?(\d+\.\d+\.\d+)(?=[-+\s]|$)/i);
  const version = betaMatch?.[1] ?? fallbackMatch?.[1] ?? "";
  if (!version) {
    return null;
  }
  return {
    version,
    displayVersion: version,
  };
}

function parseDevelopmentReleaseVersion(release: GitHubRelease): ParsedReleaseVersion | null {
  const bodyVersion = release.body?.match(/Tauri\s*版本[:：]\s*v?(\d+\.\d+\.\d+)/i)?.[1];
  const bodyBuildVersion = release.body?.match(/构建版本[:：]\s*v?([0-9A-Za-z.+-]+)/i)?.[1];
  const nameVersion = release.name?.match(
    /Legado\s+Tauri\s+v?(\d+\.\d+\.\d+)(?:\+([0-9A-Za-z.-]+))?/i,
  );
  const tagVersion = release.tag_name.match(
    /^v?(\d+\.\d+\.\d+)-([0-9]{8,14})-b([0-9A-Za-z._-]+)$/i,
  );
  const version = bodyVersion ?? nameVersion?.[1] ?? tagVersion?.[1] ?? "";
  if (!version) {
    return null;
  }

  const displayVersion = bodyBuildVersion
    ? `v${bodyBuildVersion.replace(/^v/i, "")}`
    : nameVersion?.[2]
      ? `v${version}+${nameVersion[2]}`
      : `v${version}`;
  return {
    version,
    displayVersion,
  };
}

function pickPlatformAsset(
  assets: GitHubReleaseAsset[],
  platform: AppUpdatePlatform,
  channel: AppUpdateChannel,
): AppUpdateAsset | null {
  let best: { asset: GitHubReleaseAsset; score: number } | null = null;
  for (const asset of assets) {
    const score = scorePlatformAsset(asset.name, platform, channel);
    if (score <= 0) {
      continue;
    }
    if (!best || score > best.score) {
      best = { asset, score };
    }
  }

  if (!best) {
    return null;
  }

  return {
    name: best.asset.name,
    url: best.asset.browser_download_url,
    size: best.asset.size ?? 0,
    digest: best.asset.digest ?? "",
  };
}

function scorePlatformAsset(
  assetName: string,
  platform: AppUpdatePlatform,
  channel: AppUpdateChannel,
): number {
  const name = assetName.toLowerCase();
  if (name.endsWith(".pdb")) {
    return 0;
  }

  switch (platform) {
    case "windows":
      return scoreWindowsAsset(name, channel);
    case "macos":
      return name.endsWith(".dmg") ? 100 + scoreArchitecture(name) : 0;
    case "linux":
      if (name.endsWith(".appimage")) {
        return 120;
      }
      if (name.endsWith(".deb")) {
        return 100;
      }
      if (name.endsWith(".rpm")) {
        return 90;
      }
      return 0;
    case "android":
      return name.endsWith(".apk") ? 100 + (name.includes("universal") ? 20 : 0) : 0;
    case "ios":
      return name.endsWith(".ipa") ? 100 : 0;
    case "harmony":
      return name.includes("harmony") && name.endsWith(".zip") ? 100 : 0;
    default:
      return 0;
  }
}

function scoreWindowsAsset(name: string, channel: AppUpdateChannel): number {
  if (channel === "development") {
    return name.endsWith("-windows.exe") ? 120 : 0;
  }
  if (name.endsWith("-setup.exe")) {
    return 120;
  }
  if (name.endsWith(".exe")) {
    return 110;
  }
  if (name.endsWith(".msi")) {
    return 90;
  }
  return 0;
}

function scoreArchitecture(name: string): number {
  const userAgent = typeof navigator === "undefined" ? "" : navigator.userAgent.toLowerCase();
  const armLike = /arm64|aarch64/.test(userAgent);
  if (armLike && name.includes("aarch64")) {
    return 20;
  }
  if (!armLike && (name.includes("x64") || name.includes("amd64"))) {
    return 10;
  }
  return 0;
}

function buildResult({
  channel,
  platformInfo,
  currentVersion,
  release,
  parsedVersion,
  asset,
  scannedReleaseCount,
  unavailableReason,
}: {
  channel: AppUpdateChannel;
  platformInfo: { platform: AppUpdatePlatform; label: string };
  currentVersion: string;
  release: GitHubRelease;
  parsedVersion: ParsedReleaseVersion;
  asset: AppUpdateAsset | null;
  scannedReleaseCount: number;
  unavailableReason: string;
}): AppUpdateCheckResult {
  const comparison = compareCoreVersions(parsedVersion.version, currentVersion);
  return {
    channel,
    platform: platformInfo.platform,
    platformLabel: platformInfo.label,
    currentVersion,
    latestVersion: parsedVersion.version,
    latestDisplayVersion: parsedVersion.displayVersion,
    releaseName: release.name || release.tag_name,
    releaseTag: release.tag_name,
    releaseUrl: release.html_url,
    releasePublishedAt: release.published_at ?? "",
    asset,
    hasUpdate: comparison === 1,
    scannedReleaseCount,
    unavailableReason,
  };
}
