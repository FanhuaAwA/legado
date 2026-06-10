/**
 * 西瓜播放器（xgplayer）适配器
 *
 * xgplayer 自带 HLS/DASH 插件支持；MP4 流式播放由 xgplayer-mp4 提供。
 * xgplayer-mp4 通过 HTTP Range 请求将文件切成多个分片并发拉取，
 * 比浏览器原生 <video> 整包加载节省首开时间，也支持精确 seek。
 */

import type XgPlayer from "xgplayer";
import type { IVideoPlayer, VideoPlayerEvent, VideoSource } from "../types";
import { useAppConfig } from "../../../../composables/useAppConfig";

// ─────────────────────────────────────────────────────────────────────────────
// ▌ MP4 分片下载调优参数（手动调整区）
// ─────────────────────────────────────────────────────────────────────────────

/**
 * 每个 Range 请求的分片大小（字节）。
 *
 * 越小 → 首帧越快、并发连接越多，但请求数量增加、HTTP overhead 升高。
 * 越大 → 单次传输效率高，但首帧慢、seek 后需要等待更长时间。
 *
 * 参考值：
 *   512  * 1024 = 512 KB  （弱网 / 移动端推荐）
 *   1024 * 1024 = 1  MB   （宽带家用网络推荐，当前默认）
 *   2048 * 1024 = 2  MB   （内网 / 高速 CDN 可进一步提升）
 */
const MP4_CHUNK_SIZE = 1024 * 1024 * 5; // 2 MB

/**
 * 向前预加载时长（秒）。
 *
 * 播放器会提前把当前播放位置之后 N 秒的数据下载到缓冲区。
 * 越大 → 播放越流畅，但内存占用和后台流量也越高。
 * 越小 → 节省资源，但在网速波动时容易卡顿。
 *
 * 参考值：
 *   10  秒（移动端省流模式）
 *   30  秒（宽带网络，当前默认）
 *   60  秒（高速本地播放场景）
 */
const MP4_PRELOAD_TIME = 30; // 秒

/**
 * 向后保留的缓冲时长（秒）。
 *
 * 播放位置之前的数据在超出此范围后才会从缓冲区释放。
 * 越大 → 后退 seek 不需要重新请求，但内存占用更高。
 * 越小 → 节省内存，代价是向后 seek 可能触发重新下载。
 *
 * 参考值：
 *   5  秒（移动端内存受限设备）
 *   15 秒（桌面端，当前默认）
 *   30 秒（大内存设备 / 频繁后退 seek 场景）
 */
const MP4_BUFFER_BEHIND = 30; // 秒

/**
 * 单个分片请求的超时时间（毫秒）。
 *
 * 超时后触发重试逻辑（见 MP4_RETRY_COUNT）。
 * 对于大分片（> 1 MB）或弱网，应相应增大此值。
 *
 * 参考值：
 *   8000  ms（快速网络，当前默认）
 *   15000 ms（普通网络）
 *   30000 ms（弱网 / 移动数据）
 */
const MP4_REQUEST_TIMEOUT = 20000; // ms

/**
 * 请求失败后的最大重试次数。
 *
 * 每次重试前等待 MP4_RETRY_DELAY 毫秒。
 * 超出后播放器抛出 error 事件。
 *
 * 参考值：
 *   2（快速失败场景）
 *   3（当前默认，兼顾体验与速度）
 *   5（弱网容错优先）
 */
const MP4_RETRY_COUNT = 3;

/**
 * 相邻两次重试之间的间隔（毫秒）。
 *
 * 建议与 MP4_REQUEST_TIMEOUT 配合调整，避免重试太频繁打满服务器连接。
 *
 * 参考值：
 *   500  ms（内网场景）
 *   1000 ms（当前默认）
 *   2000 ms（弱网 / 限流 CDN）
 */
const MP4_RETRY_DELAY = 500; // ms

/** xgplayer 事件名映射：统一接口事件 → xgplayer 实际事件 */
const EVENT_MAP: Record<VideoPlayerEvent, string> = {
  play: "play",
  pause: "pause",
  ended: "ended",
  timeupdate: "timeupdate",
  error: "error",
  volumechange: "volumechange",
  ratechange: "ratechange",
  loadedmetadata: "loadedmetadata",
  waiting: "waiting",
  canplay: "canplay",
};

export class XgplayerAdapter implements IVideoPlayer {
  private player: InstanceType<typeof XgPlayer> | null = null;
  private container: HTMLElement | null = null;

  async mount(container: HTMLElement, source: VideoSource): Promise<void> {
    this.container = container;
    await this.initPlayer(source);
  }

  private async initPlayer(source: VideoSource): Promise<void> {
    const { default: Player } = await import("xgplayer");
    await import("xgplayer/dist/index.min.css");

    if (!this.container) {
      return;
    }

    const { videoXgDownload } = useAppConfig();

    // 创建播放器容器 div
    const el = document.createElement("div");
    this.container.appendChild(el);

    const config: Record<string, unknown> = {
      el,
      url: source.url,
      fluid: true,
      playbackRate: [0.5, 0.75, 1, 1.25, 1.5, 2, 3],
      playsinline: true,
      autoplay: false,
      download: videoXgDownload.value,
    };

    // HLS 支持
    if (source.type === "hls") {
      try {
        const { HlsPlugin } = await import("xgplayer-hls");
        config.plugins = [HlsPlugin];
        config.hls = source.headers ? { headers: source.headers } : undefined;
      } catch {
        // HLS 插件未安装，回退到原生
        config.url = source.url;
      }
    }

    // MP4 支持（xgplayer-mp4：MSE 解封装 + Range 分片下载）
    //
    // xgplayer-mp4 工作原理：
    //   1. 发送 HEAD 请求探测文件大小与是否支持 Range；
    //   2. 优先下载 moov box（文件元信息），解析轨道/时长；
    //   3. 按 MP4_CHUNK_SIZE 将文件切分，通过 Range: bytes=x-y 并发拉取；
    //   4. 通过 MSE (Media Source Extensions) 逐块喂给 <video>，实现边下边播。
    //
    // 当 source.type 未指定时也默认走此路径，兼容绝大多数直链视频场景。
    if (source.type === "mp4" || source.type === undefined) {
      try {
        const { default: Mp4Plugin } = await import("xgplayer-mp4");
        config.plugins = [...((config.plugins as unknown[]) ?? []), Mp4Plugin];
        const mp4PluginConfig = {
          // ── 分片大小 ──────────────────────────────────────────────────────
          // 每次 Range 请求下载的字节数，直接影响并发粒度与首帧速度。
          // 调小 → 首帧快，请求数多；调大 → 吞吐高，首帧稍慢。
          chunkSize: MP4_CHUNK_SIZE,

          // ── 预加载缓冲 ────────────────────────────────────────────────────
          // 向前预加载的秒数，越大越流畅，但内存占用升高。
          preloadTime: MP4_PRELOAD_TIME,

          // ── 向后缓冲保留 ──────────────────────────────────────────────────
          // 已播放片段在内存中保留的秒数，保证向后 seek 无需重新请求。
          bufferBehind: MP4_BUFFER_BEHIND,

          // ── 超时 & 重试 ───────────────────────────────────────────────────
          // 单分片请求超时时间（ms），超时后触发重试。
          timeout: MP4_REQUEST_TIMEOUT,
          // 最大重试次数，超出后向上层抛 error 事件。
          retryCount: MP4_RETRY_COUNT,
          // 相邻重试间隔（ms）。
          retryDelay: MP4_RETRY_DELAY,

          // ── 自定义请求头 ──────────────────────────────────────────────────
          // xgplayer-mp4 v3 通过 mp4plugin.reqOptions.headers 透传自定义请求头。
          reqOptions: {
            method: "GET",
            mode: "cors",
            ...(source.headers ? { headers: source.headers } : {}),
          },
        };
        config.mp4plugin = mp4PluginConfig;
      } catch {
        // xgplayer-mp4 插件加载失败，退回浏览器原生 MP4 播放（无分片优化）
      }
    }

    this.player = new Player(config as ConstructorParameters<typeof Player>[0]);

    // 字幕
    if (source.subtitles?.length) {
      // xgplayer 通过配置设置字幕，运行时设置需依赖其 API
      // 回退：通过 video 元素添加 track
      const videoEl = this.player.video;
      if (videoEl instanceof HTMLElement) {
        for (const sub of source.subtitles) {
          const track = document.createElement("track");
          track.kind = "subtitles";
          track.label = sub.label;
          track.srclang = sub.srclang ?? "zh";
          track.src = sub.url;
          if (sub.default) {
            track.default = true;
          }
          videoEl.appendChild(track);
        }
      }
    }
  }

  play(): void {
    this.player?.play();
  }
  pause(): void {
    this.player?.pause();
  }
  seek(seconds: number): void {
    if (this.player) {
      this.player.currentTime = seconds;
    }
  }
  getCurrentTime(): number {
    return this.player?.currentTime ?? 0;
  }
  getDuration(): number {
    return this.player?.duration ?? 0;
  }
  setVolume(v: number): void {
    if (this.player) {
      this.player.volume = v;
    }
  }
  getVolume(): number {
    return this.player?.volume ?? 1;
  }
  setPlaybackRate(rate: number): void {
    if (this.player) {
      this.player.playbackRate = rate;
    }
  }
  getPlaybackRate(): number {
    return this.player?.playbackRate ?? 1;
  }

  enterFullscreen(): void {
    void this.player?.getFullscreen();
  }
  exitFullscreen(): void {
    void this.player?.exitFullscreen();
  }
  isFullscreen(): boolean {
    return this.player?.fullscreen ?? false;
  }

  on(event: VideoPlayerEvent, handler: (...args: unknown[]) => void): void {
    this.player?.on(EVENT_MAP[event] ?? event, handler);
  }

  off(event: VideoPlayerEvent, handler: (...args: unknown[]) => void): void {
    this.player?.off(EVENT_MAP[event] ?? event, handler);
  }

  destroy(): void {
    if (this.player) {
      // 先暂停，再清空 src，确保音频立即停止（xgplayer.destroy() 异步释放资源可能有延迟）
      this.player.pause();
      const videoEl = this.player.video as HTMLVideoElement | undefined;
      if (videoEl) {
        videoEl.pause();
        videoEl.removeAttribute("src");
        try {
          videoEl.load();
        } catch {
          /* ignore */
        }
      }
      this.player.destroy();
      this.player = null;
    }
    this.container = null;
  }
}
