// ==UserScript==
// @name         自定义背景图
// @namespace    com.legado.extensions
// @version      1.0.0
// @description  上传多张图片，并把每张图片注册为阅读器背景选项
// @author       designer_x
// @category     主题风格
// @match        *
// @grant        none
// @run-at       document-start
// @enabled      false
// ==/UserScript==

function normalizeImages(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter(function (item) {
    return (
      item &&
      typeof item === "object" &&
      typeof item.id === "string" &&
      typeof item.dataUrl === "string"
    );
  });
}

function imageName(image, index) {
  var raw =
    typeof image.name === "string" && image.name.trim()
      ? image.name.trim()
      : "背景图 " + (index + 1);
  return raw.replace(/\.[^.]+$/, "").slice(0, 18) || "背景图 " + (index + 1);
}

function cssUrl(value) {
  return 'url("' + String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '")';
}

function clampPercent(value, fallback) {
  var next = Number(value);
  if (!isFinite(next)) {
    next = fallback;
  }
  return Math.max(0, Math.min(100, next));
}

function buildPatch(image, settings, preview) {
  var mask = clampPercent(settings.mask, preview ? 36 : 28) / 100;
  var baseColor = settings.baseColor || "#f6f2ea";
  var textColor = settings.textColor || "#1f2937";
  var fit = settings.fit || "cover";
  var position = settings.position || "center";
  var repeat = settings.repeat || "no-repeat";
  var overlay = "linear-gradient(rgba(255,255,255," + mask + "), rgba(255,255,255," + mask + "))";
  return {
    backgroundColor: baseColor,
    textColor: textColor,
    selectionColor: settings.selectionColor || "#d6bb8a",
    backgroundImage: overlay + ", " + cssUrl(image.dataUrl),
    backgroundSize: "cover, " + fit,
    backgroundPosition: "center, " + position,
    backgroundRepeat: "no-repeat, " + repeat,
    backgroundBlendMode: "normal, normal",
  };
}

legado.registerPlugin({
  id: "reader-uploaded-background-images",
  settings: {
    defaults: {
      images: [],
      fit: "cover",
      position: "center",
      repeat: "no-repeat",
      mask: 28,
      baseColor: "#f6f2ea",
      textColor: "#1f2937",
      selectionColor: "#d6bb8a",
    },
    schema: function (context) {
      var count = normalizeImages(context.values.images).length;
      return [
        {
          type: "image-list",
          key: "images",
          label: "背景图片",
          description: "上传后，每张图片都会出现在阅读设置的“背景”选项中。单张图片上限 8 MB。",
          placeholder: "支持 PNG、JPG、WebP、GIF、SVG，可拖拽到此上传",
          max: 40,
        },
        {
          type: "select",
          key: "fit",
          label: "铺放方式",
          options: [
            { label: "填满", value: "cover" },
            { label: "完整显示", value: "contain" },
            { label: "原始尺寸", value: "auto" },
          ],
        },
        {
          type: "select",
          key: "position",
          label: "图片位置",
          options: [
            { label: "居中", value: "center" },
            { label: "顶部", value: "center top" },
            { label: "底部", value: "center bottom" },
            { label: "左上", value: "left top" },
            { label: "右上", value: "right top" },
          ],
        },
        {
          type: "select",
          key: "repeat",
          label: "重复方式",
          options: [
            { label: "不重复", value: "no-repeat" },
            { label: "平铺", value: "repeat" },
            { label: "横向平铺", value: "repeat-x" },
            { label: "纵向平铺", value: "repeat-y" },
          ],
        },
        {
          type: "slider",
          key: "mask",
          label: "白色蒙层",
          min: 0,
          max: 80,
          step: 5,
        },
        { type: "color", key: "baseColor", label: "底色" },
        { type: "color", key: "textColor", label: "文字颜色" },
        { type: "color", key: "selectionColor", label: "选中颜色" },
        {
          type: "info",
          label: "背景注册",
          description: count
            ? "当前已注册 " + count + " 个自定义背景，可回到阅读页设置菜单的“背景”中选择。"
            : "上传图片后会自动注册背景选项。",
        },
      ];
    },
  },
  backgrounds: function (context) {
    var settings = context.values;
    return normalizeImages(settings.images).map(function (image, index) {
      return {
        id: "uploaded-" + image.id,
        name: imageName(image, index),
        description: "来自自定义背景图插件的上传图片",
        preview: function () {
          return buildPatch(image, settings, true);
        },
        resolve: function (_, api) {
          var latestSettings = api.settings.getAll();
          var latestImage = normalizeImages(latestSettings.images).find(function (item) {
            return item.id === image.id;
          });
          return buildPatch(latestImage || image, latestSettings, false);
        },
      };
    });
  },
});
