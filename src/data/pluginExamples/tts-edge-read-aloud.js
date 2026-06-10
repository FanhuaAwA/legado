// ==UserScript==
// @name         Edge Read Aloud TTS 引擎
// @namespace    com.legado.plugins.tts.edge-read-aloud
// @version      1.0.0
// @description  示例：把旧 Edge Read Aloud 在线协议封装为可选 TTS 插件引擎。
// @author       Legado
// @category     TTS
// @match        *
// @grant        none
// @run-at       document-idle
// @enabled      false
// ==/UserScript==

legado.registerPlugin({
  id: "edge-read-aloud-tts",
  name: "Edge Read Aloud TTS 引擎",
  setup: function (api) {
    var WSS_BASE = "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1";
    var DEFAULT_TOKEN = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
    var DEFAULT_CHROMIUM_VERSION = "143.0.3650.75";
    var currentAudio = null;
    var activeSockets = [];
    var currentUrl = "";

    var voices = [
      { id: "zh-CN-XiaoxiaoNeural", name: "晓晓", language: "zh-CN" },
      { id: "zh-CN-YunxiNeural", name: "云希", language: "zh-CN" },
      { id: "zh-CN-YunjianNeural", name: "云健", language: "zh-CN" },
      { id: "zh-CN-XiaoyiNeural", name: "晓伊", language: "zh-CN" },
      { id: "zh-CN-YunyangNeural", name: "云扬", language: "zh-CN" },
      { id: "zh-CN-XiaochenNeural", name: "晓辰", language: "zh-CN" },
      { id: "zh-CN-XiaohanNeural", name: "晓涵", language: "zh-CN" },
      { id: "zh-CN-XiaomengNeural", name: "晓梦", language: "zh-CN" },
      { id: "zh-CN-XiaomoNeural", name: "晓墨", language: "zh-CN" },
      { id: "zh-CN-XiaoqiuNeural", name: "晓秋", language: "zh-CN" },
      { id: "zh-CN-XiaoruiNeural", name: "晓睿", language: "zh-CN" },
      { id: "zh-CN-XiaoshuangNeural", name: "晓双", language: "zh-CN" },
      { id: "zh-CN-XiaoxuanNeural", name: "晓萱", language: "zh-CN" },
      { id: "zh-CN-XiaoyanNeural", name: "晓颜", language: "zh-CN" },
      { id: "zh-CN-XiaoyouNeural", name: "晓悠", language: "zh-CN" },
      { id: "zh-CN-XiaozhenNeural", name: "晓甄", language: "zh-CN" },
    ];

    function stopAudio() {
      if (currentAudio) {
        currentAudio.pause();
        currentAudio.onended = null;
        currentAudio.onerror = null;
        currentAudio = null;
      }
      if (currentUrl) {
        URL.revokeObjectURL(currentUrl);
        currentUrl = "";
      }
    }

    function stopCurrent() {
      stopAudio();
      for (var socketIndex = 0; socketIndex < activeSockets.length; socketIndex++) {
        try {
          activeSockets[socketIndex].close();
        } catch (error) {
          api.log("关闭 Edge TTS socket 失败", error);
        }
      }
      activeSockets = [];
    }

    function uuidSimple() {
      var bytes = new Uint8Array(16);
      crypto.getRandomValues(bytes);
      bytes[6] = (bytes[6] & 0x0f) | 0x40;
      bytes[8] = (bytes[8] & 0x3f) | 0x80;
      return Array.from(bytes)
        .map(function (byte) {
          return byte.toString(16).padStart(2, "0");
        })
        .join("");
    }

    async function sha256Upper(text) {
      if (!crypto.subtle) {
        throw new Error("当前环境不支持 crypto.subtle，无法生成 Edge TTS 鉴权参数");
      }
      var bytes = new TextEncoder().encode(text);
      var hash = await crypto.subtle.digest("SHA-256", bytes);
      return Array.from(new Uint8Array(hash))
        .map(function (byte) {
          return byte.toString(16).padStart(2, "0");
        })
        .join("")
        .toUpperCase();
    }

    async function generateSecMsGec(token) {
      var unixSeconds = Math.floor(Date.now() / 1000);
      var rounded = Math.floor((unixSeconds + 11644473600) / 300) * 300;
      var ticks = (BigInt(rounded) * 10000000n).toString();
      return sha256Upper(ticks + token);
    }

    function pad(value) {
      return String(value).padStart(2, "0");
    }

    function dateString() {
      var date = new Date();
      var days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
      var months = [
        "Jan",
        "Feb",
        "Mar",
        "Apr",
        "May",
        "Jun",
        "Jul",
        "Aug",
        "Sep",
        "Oct",
        "Nov",
        "Dec",
      ];
      return (
        days[date.getUTCDay()] +
        " " +
        months[date.getUTCMonth()] +
        " " +
        pad(date.getUTCDate()) +
        " " +
        date.getUTCFullYear() +
        " " +
        pad(date.getUTCHours()) +
        ":" +
        pad(date.getUTCMinutes()) +
        ":" +
        pad(date.getUTCSeconds()) +
        " GMT+0000 (Coordinated Universal Time)"
      );
    }

    function xmlEscape(text) {
      return (
        String(text)
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;")
          .replace(/"/g, "&quot;")
          .replace(/'/g, "&apos;")
          // oxlint-disable-next-line no-control-regex -- XML 1.0 forbids C0 control chars in SSML.
          .replace(new RegExp("[\\u0000-\\u0008\\u000b\\u000c\\u000e-\\u001f]", "g"), " ")
      );
    }

    function normalizeVoice(voice) {
      if (voice.indexOf("Microsoft Server Speech Text to Speech Voice") === 0) {
        return voice;
      }
      var parts = voice.split("-");
      if (parts.length < 3) {
        return voice;
      }
      return (
        "Microsoft Server Speech Text to Speech Voice (" +
        parts[0] +
        "-" +
        parts[1] +
        ", " +
        parts.slice(2).join("-") +
        ")"
      );
    }

    function edgeRate(rate) {
      var value = Math.round((Number(rate || 1) - 1) * 100);
      return (value >= 0 ? "+" : "") + value + "%";
    }

    function makeSsml(text, voice, rate) {
      return (
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' " +
        "xmlns:mstts='https://www.w3.org/2001/mstts' xml:lang='zh-CN'>" +
        "<voice name='" +
        normalizeVoice(voice) +
        "'><prosody pitch='+0Hz' rate='" +
        rate +
        "' volume='+0%'>" +
        xmlEscape(text) +
        "</prosody></voice></speak>"
      );
    }

    function speechConfigMessage() {
      return (
        "X-Timestamp:" +
        dateString() +
        "\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n" +
        '{"context":{"synthesis":{"audio":{"metadataoptions":{"sentenceBoundaryEnabled":"false","wordBoundaryEnabled":"false"},"outputFormat":"audio-24khz-48kbitrate-mono-mp3"}}}}'
      );
    }

    function ssmlMessage(ssml) {
      return (
        "X-RequestId:" +
        uuidSimple() +
        "\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:" +
        dateString() +
        "Z\r\nPath:ssml\r\n\r\n" +
        ssml
      );
    }

    function parseAudio(data) {
      var bytes = new Uint8Array(data);
      if (bytes.length < 2) {
        return null;
      }
      var headerLength = (bytes[0] << 8) | bytes[1];
      var headerEnd = 2 + headerLength;
      if (headerEnd > bytes.length) {
        return null;
      }
      var header = new TextDecoder().decode(bytes.slice(2, headerEnd)).toLowerCase();
      if (header.indexOf("path:audio") >= 0 && header.indexOf("audio/mpeg") >= 0) {
        return bytes.slice(headerEnd);
      }
      return null;
    }

    async function synthesize(text, context) {
      var token = api.settings.get("trustedClientToken", DEFAULT_TOKEN);
      var version = api.settings.get("chromiumVersion", DEFAULT_CHROMIUM_VERSION);
      var connectionId = uuidSimple();
      var secMsGec = await generateSecMsGec(token);
      var url =
        WSS_BASE +
        "?TrustedClientToken=" +
        encodeURIComponent(token) +
        "&ConnectionId=" +
        connectionId +
        "&Sec-MS-GEC=" +
        secMsGec +
        "&Sec-MS-GEC-Version=1-" +
        encodeURIComponent(version);
      var voice = context.voiceId || api.settings.get("voice", "zh-CN-XiaoxiaoNeural");
      var rate = edgeRate(context.rate);

      return new Promise(function (resolve, reject) {
        var chunks = [];
        var settled = false;
        var socket = new WebSocket(url);
        activeSockets.push(socket);
        socket.binaryType = "arraybuffer";

        function settle(fn, value) {
          if (settled) {
            return;
          }
          settled = true;
          activeSockets = activeSockets.filter(function (item) {
            return item !== socket;
          });
          try {
            socket.close();
          } catch (error) {
            api.log("关闭 Edge TTS socket 失败", error);
          }
          fn(value);
        }

        context.signal.addEventListener(
          "abort",
          function () {
            settle(resolve, null);
          },
          { once: true },
        );

        socket.onopen = function () {
          socket.send(speechConfigMessage());
          socket.send(ssmlMessage(makeSsml(text, voice, rate)));
        };
        socket.onerror = function () {
          settle(reject, new Error("Edge TTS WebSocket 连接失败"));
        };
        socket.onclose = function () {
          if (!settled && chunks.length > 0) {
            settle(resolve, new Blob(chunks, { type: "audio/mpeg" }));
          } else if (!settled) {
            settle(reject, new Error("Edge TTS 未返回音频数据"));
          }
        };
        socket.onmessage = function (event) {
          if (typeof event.data === "string") {
            if (event.data.indexOf("Path:turn.end") >= 0) {
              settle(resolve, new Blob(chunks, { type: "audio/mpeg" }));
            }
            return;
          }
          var audio = parseAudio(event.data);
          if (audio && audio.length > 0) {
            chunks.push(audio);
          }
        };
      });
    }

    async function playBlob(blob, context) {
      if (!blob || context.signal.aborted) {
        return;
      }
      stopAudio();
      currentUrl = URL.createObjectURL(blob);
      currentAudio = new Audio(currentUrl);
      currentAudio.playbackRate = context.rate || 1;
      currentAudio.volume = context.volume == null ? 1 : context.volume;

      await new Promise(function (resolve, reject) {
        context.signal.addEventListener(
          "abort",
          function () {
            stopAudio();
            resolve();
          },
          { once: true },
        );
        currentAudio.onended = function () {
          stopAudio();
          resolve();
        };
        currentAudio.onerror = function () {
          stopAudio();
          reject(new Error("Edge TTS 音频播放失败"));
        };
        currentAudio.play().catch(reject);
      });
    }

    return {
      settings: {
        defaults: {
          voice: "zh-CN-XiaoxiaoNeural",
          trustedClientToken: DEFAULT_TOKEN,
          chromiumVersion: DEFAULT_CHROMIUM_VERSION,
        },
        schema: [
          {
            type: "select",
            key: "voice",
            label: "默认语音",
            options: voices.map(function (voice) {
              return {
                label: voice.name + " · " + voice.language,
                value: voice.id,
              };
            }),
          },
          { type: "text", key: "chromiumVersion", label: "Chromium 版本" },
          {
            type: "text",
            key: "trustedClientToken",
            label: "Trusted Client Token",
          },
        ],
      },
      ttsEngines: [
        {
          id: "edge-read-aloud",
          name: "Edge Read Aloud",
          description: "旧 Edge 在线朗读协议示例插件",
          category: "TTS",
          getVoices: function () {
            return voices;
          },
          preload: function (context) {
            return synthesize(context.text, context);
          },
          speak: async function (context) {
            var blob = context.preloaded || (await synthesize(context.text, context));
            await playBlob(blob, context);
          },
          stop: function () {
            stopCurrent();
          },
          previewVoice: async function (voiceId) {
            var controller = new AbortController();
            var context = {
              text: "这是一段 Edge 在线朗读试听。",
              voiceId: voiceId,
              language: "zh-CN",
              rate: 1,
              pitch: 1,
              volume: 1,
              signal: controller.signal,
            };
            var blob = await synthesize(context.text, context);
            await playBlob(blob, context);
          },
        },
      ],
      dispose: function () {
        stopCurrent();
      },
    };
  },
});
