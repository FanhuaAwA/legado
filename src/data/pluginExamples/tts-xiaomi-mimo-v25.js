// ==UserScript==
// @name         小米 MiMo V2.5 TTS 引擎
// @namespace    com.legado.plugins.tts.xiaomi-mimo-v25
// @version      1.0.0
// @description  内置：调用 Xiaomi MiMo V2.5 TTS API 进行在线语音合成，需在插件设置中填写 API Key。
// @author       Legado
// @category     TTS
// @match        *
// @grant        none
// @run-at       document-idle
// @enabled      true
// ==/UserScript==

legado.registerPlugin({
  id: "xiaomi-mimo-v25-tts",
  name: "小米 MiMo V2.5 TTS 引擎",
  setup: function (api) {
    var DEFAULT_ENDPOINT = "https://api.xiaomimimo.com/v1/chat/completions";
    var DEFAULT_MODEL = "mimo-v2.5-tts";
    var currentAudio = null;
    var currentUrl = "";

    var models = [
      {
        id: "mimo-v2.5-tts",
        name: "MiMo-V2.5-TTS（预置音色）",
      },
      {
        id: "mimo-v2.5-tts-voicedesign",
        name: "MiMo-V2.5-TTS-VoiceDesign（文本设计音色）",
      },
      {
        id: "mimo-v2.5-tts-voiceclone",
        name: "MiMo-V2.5-TTS-VoiceClone（音色复刻）",
      },
    ];

    var voices = [
      { id: "mimo_default", name: "MiMo 默认", language: "auto" },
      { id: "冰糖", name: "冰糖", language: "zh-CN" },
      { id: "茉莉", name: "茉莉", language: "zh-CN" },
      { id: "苏打", name: "苏打", language: "zh-CN" },
      { id: "白桦", name: "白桦", language: "zh-CN" },
      { id: "Mia", name: "Mia", language: "en" },
      { id: "Chloe", name: "Chloe", language: "en" },
      { id: "Milo", name: "Milo", language: "en" },
      { id: "Dean", name: "Dean", language: "en" },
    ];

    function stopCurrent() {
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

    function settingString(key, fallback) {
      var value = api.settings.get(key, fallback);
      if (typeof value !== "string") {
        return fallback;
      }
      return value.trim();
    }

    function settingNumber(key, fallback) {
      var value = Number(api.settings.get(key, fallback));
      return Number.isFinite(value) ? value : fallback;
    }

    function settingBoolean(key, fallback) {
      return api.settings.get(key, fallback) === true;
    }

    function currentModel() {
      var model = settingString("model", DEFAULT_MODEL);
      return models.some(function (item) {
        return item.id === model;
      })
        ? model
        : DEFAULT_MODEL;
    }

    function ensureApiKey() {
      var apiKey = settingString("apiKey", "");
      if (!apiKey) {
        throw new Error("请先在「小米 MiMo V2.5 TTS 引擎」插件设置中填写 API Key");
      }
      return apiKey;
    }

    function speedInstruction(rate) {
      var value = Number(rate || 1);
      if (!Number.isFinite(value)) {
        return "";
      }
      if (value >= 1.15) {
        return "语速偏快，约为正常语速的 " + value.toFixed(1) + " 倍。";
      }
      if (value <= 0.85) {
        return "语速放慢，约为正常语速的 " + value.toFixed(1) + " 倍。";
      }
      return "";
    }

    function pitchInstruction(pitch) {
      var value = Number(pitch || 1);
      if (!Number.isFinite(value)) {
        return "";
      }
      if (value >= 1.12) {
        return "音调略高，声音更明亮。";
      }
      if (value <= 0.88) {
        return "音调略低，声音更沉稳。";
      }
      return "";
    }

    function buildStyleInstruction(context) {
      var parts = [];
      var stylePrompt = settingString("stylePrompt", "");
      var speed = speedInstruction(context.rate);
      var pitch = pitchInstruction(context.pitch);
      if (stylePrompt) {
        parts.push(stylePrompt);
      }
      if (speed) {
        parts.push(speed);
      }
      if (pitch) {
        parts.push(pitch);
      }
      return parts.join("\n");
    }

    function buildMessages(text, context, model) {
      var style = buildStyleInstruction(context);
      if (model === "mimo-v2.5-tts-voicedesign") {
        var designPrompt = settingString("voiceDesignPrompt", "");
        if (!designPrompt) {
          throw new Error("VoiceDesign 模型需要先在插件设置中填写音色设计描述");
        }
        return [
          {
            role: "user",
            content: style ? designPrompt + "\n\n发音风格：" + style : designPrompt,
          },
          { role: "assistant", content: text },
        ];
      }
      return [
        { role: "user", content: style },
        { role: "assistant", content: text },
      ];
    }

    function buildAudioOptions(context, model) {
      var audio = { format: "wav" };
      if (model === "mimo-v2.5-tts") {
        audio.voice = context.voiceId || settingString("voice", "mimo_default");
      } else if (model === "mimo-v2.5-tts-voicedesign") {
        audio.optimize_text_preview = settingBoolean("optimizeTextPreview", false);
      } else if (model === "mimo-v2.5-tts-voiceclone") {
        var cloneVoice = settingString("voiceCloneData", "");
        if (!cloneVoice) {
          throw new Error(
            "VoiceClone 模型需要先在插件设置中填写 data:audio/...;base64,... 音频样本",
          );
        }
        audio.voice = cloneVoice;
      }
      return audio;
    }

    function decodeAudioBlob(base64, mime) {
      var clean = String(base64 || "").replace(/^data:[^,]+,/, "");
      var binary = atob(clean);
      var chunkSize = 8192;
      var chunks = [];
      for (var offset = 0; offset < binary.length; offset += chunkSize) {
        var slice = binary.slice(offset, offset + chunkSize);
        var bytes = new Uint8Array(slice.length);
        for (var index = 0; index < slice.length; index++) {
          bytes[index] = slice.charCodeAt(index);
        }
        chunks.push(bytes);
      }
      return new Blob(chunks, { type: mime || "audio/wav" });
    }

    function parseAudioData(body) {
      var data;
      try {
        data = JSON.parse(body);
      } catch {
        throw new Error("MiMo TTS 返回内容不是有效 JSON");
      }

      if (data && data.error) {
        throw new Error(data.error.message || data.error.code || "MiMo TTS 请求失败");
      }

      var choice = data && data.choices && data.choices[0];
      var message = choice && choice.message;
      var audio = message && message.audio;
      var audioData = audio && audio.data;
      if (!audioData) {
        throw new Error("MiMo TTS 未返回音频数据");
      }
      return audioData;
    }

    async function synthesize(text, context) {
      if (context.signal.aborted) {
        return null;
      }

      var model = currentModel();
      var payload = {
        model: model,
        messages: buildMessages(text, context, model),
        audio: buildAudioOptions(context, model),
      };
      var response = await api.http.request({
        url: settingString("endpoint", DEFAULT_ENDPOINT),
        method: "POST",
        headers: {
          "api-key": ensureApiKey(),
          "Content-Type": "application/json",
        },
        body: JSON.stringify(payload),
        timeoutSecs: settingNumber("timeoutSecs", 45),
      });

      if (context.signal.aborted) {
        return null;
      }
      if (response.status < 200 || response.status >= 300) {
        var message = "MiMo TTS HTTP " + response.status;
        try {
          var errorBody = JSON.parse(response.body);
          if (errorBody && errorBody.error && errorBody.error.message) {
            message += ": " + errorBody.error.message;
          }
        } catch {
          if (response.body) {
            message += ": " + response.body.slice(0, 200);
          }
        }
        throw new Error(message);
      }

      return decodeAudioBlob(parseAudioData(response.body), "audio/wav");
    }

    async function playBlob(blob, context) {
      if (!blob || context.signal.aborted) {
        return;
      }
      stopCurrent();
      currentUrl = URL.createObjectURL(blob);
      currentAudio = new Audio(currentUrl);
      currentAudio.playbackRate = context.rate || 1;
      currentAudio.volume = context.volume == null ? 1 : context.volume;

      await new Promise(function (resolve, reject) {
        context.signal.addEventListener(
          "abort",
          function () {
            stopCurrent();
            resolve();
          },
          { once: true },
        );
        currentAudio.onended = function () {
          stopCurrent();
          resolve();
        };
        currentAudio.onerror = function () {
          stopCurrent();
          reject(new Error("MiMo TTS 音频播放失败"));
        };
        currentAudio.play().catch(reject);
      });
    }

    return {
      settings: {
        defaults: {
          apiKey: "",
          endpoint: DEFAULT_ENDPOINT,
          model: DEFAULT_MODEL,
          voice: "mimo_default",
          stylePrompt: "自然、清晰、适合长篇小说朗读。根据文本内容保留情绪，不额外解释。",
          voiceDesignPrompt: "",
          voiceCloneData: "",
          optimizeTextPreview: false,
          timeoutSecs: 45,
        },
        schema: function (context) {
          var model = context.values.model || DEFAULT_MODEL;
          return [
            {
              type: "info",
              label: "Xiaomi MiMo V2.5 TTS",
              description:
                "需要在小米 MiMo 开放平台创建 API Key。朗读文本会作为 assistant message 发送到接口。",
            },
            {
              type: "password",
              key: "apiKey",
              label: "API Key",
              placeholder: "填入 MIMO_API_KEY",
            },
            {
              type: "text",
              key: "endpoint",
              label: "接口地址",
              placeholder: DEFAULT_ENDPOINT,
            },
            {
              type: "select",
              key: "model",
              label: "模型",
              options: models.map(function (item) {
                return { label: item.name, value: item.id };
              }),
            },
            {
              type: "select",
              key: "voice",
              label: "预置音色",
              hidden: function () {
                return model !== "mimo-v2.5-tts";
              },
              options: voices.map(function (voice) {
                return {
                  label: voice.name + " · " + voice.language,
                  value: voice.id,
                };
              }),
            },
            {
              type: "textarea",
              key: "stylePrompt",
              label: "朗读风格指令",
              rows: 3,
              placeholder: "例如：温柔、自然、适合小说旁白。",
            },
            {
              type: "textarea",
              key: "voiceDesignPrompt",
              label: "音色设计描述",
              rows: 4,
              hidden: function () {
                return model !== "mimo-v2.5-tts-voicedesign";
              },
              placeholder: "例如：一位年轻女性，声音温柔治愈，语速缓慢，适合睡前故事。",
            },
            {
              type: "switch",
              key: "optimizeTextPreview",
              label: "VoiceDesign 自动润色文本",
              hidden: function () {
                return model !== "mimo-v2.5-tts-voicedesign";
              },
            },
            {
              type: "textarea",
              key: "voiceCloneData",
              label: "音色复刻样本 Base64",
              rows: 4,
              hidden: function () {
                return model !== "mimo-v2.5-tts-voiceclone";
              },
              placeholder: "data:audio/mpeg;base64,... 或 data:audio/wav;base64,...",
            },
            {
              type: "number",
              key: "timeoutSecs",
              label: "请求超时（秒）",
              min: 10,
              max: 180,
              step: 5,
            },
          ];
        },
      },
      ttsEngines: [
        {
          id: "xiaomi-mimo-v25",
          name: "Xiaomi MiMo V2.5",
          description: "调用 Xiaomi MiMo V2.5 TTS 系列 API 生成语音",
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
          pause: function () {
            if (currentAudio) {
              currentAudio.pause();
            }
          },
          resume: function () {
            if (currentAudio) {
              currentAudio.play().catch(function (error) {
                api.log("MiMo TTS 恢复播放失败", error);
              });
            }
          },
          previewVoice: async function (voiceId) {
            var controller = new AbortController();
            var context = {
              text: "这是一段小米 MiMo V2.5 语音合成试听。",
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
