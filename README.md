# Cloudflare Worker Image

使用 Cloudflare Worker 处理图片, 依赖 Photon，支持缩放、剪裁、水印、滤镜等功能。

---

![Cloudflare Workers](https://img.shields.io/badge/Cloudflare-F69652?style=flat&logo=cloudflare&logoColor=white)
![GitHub License](https://img.shields.io/github/license/ccbikai/cloudflare-worker-image)
![GitHub Repo stars](https://img.shields.io/github/stars/ccbikai/cloudflare-worker-image)

> 已经适配了 Vercel Edge, 见 <https://github.com/ccbikai/vercel-edge-image> 。

## 支持特性

1. 支持 PNG、JPEG、BMP、ICO、TIFF 格式图片处理
2. 可输出 JPEG、PNG、WEBP 格式图片，默认输出 WEBP 格式图片
3. 支持管道操作，可以执行多个操作
4. 支持 Cloudflare 缓存
5. 支持图片地址白名单，防滥用
6. 异常降级，如果处理失败返回原图（异常场景不缓存）

## 部署方式

```sh
# patch 功能依赖 pnpm, 如果不使用 pnpm, 需要自己处理 patch-package https://www.npmjs.com/package/patch-package
npm i -g pnpm

# 克隆此项目
git clone https://github.com/ccbikai/cloudflare-worker-image.git
cd cloudflare-worker-image

# 安装依赖
pnpm install

# 修改白名单配置，改为图片域名或者留空不限制图片地址
vi wrangler.toml # WHITE_LIST

# 发布
npm run deploy
```

## 使用方式

修改域名和参数即可使用, 参考：<https://image.miantiao.me/?url=https%3A%2F%2Fstatic.miantiao.me%2Fshare%2FMTyerw%2Fbanner-2048.jpeg&action=resize!830,400,2>

### 参数说明

url:
> 原图地址，需要使用 encodeURIComponent 编码

action:
> 操作指令, 支持 [Photon](https://docs.rs/photon-rs/latest/photon_rs/) 各种操作指令，指令与参数直接使用`!`分割，参考 `resize!830,400,2`
>
> 支持管道操作，多个操作指令使用`|`分割，参考 `resize!830,400,2|watermark!https%3A%2F%2Fstatic.miantiao.me%2Fshare%2F6qIq4w%2FFhSUzU.png,10,10`
>
> 如果参数中有 URL 或其他特殊字符，需要使用 encodeURIComponent 编码 URL 和 特殊字符

format:
> 输出图片格式，支持：`jpg,webp,png`，可选，默认 webp

quality:
> 图片质量，1-100 只有 webp 和 jpg 格式支持，可选，默认 99

## 演示

### 缩放+旋转+文字水印

![demo](https://image.miantiao.me/?url=https%3A%2F%2Fstatic.miantiao.me%2Fshare%2FMTyerw%2Fbanner-2048.jpeg&action=resize!830,400,2%7Crotate!180%7Cdraw_text!miantiao.me,10,10)

由于 Github 会缓存图片，请前往我博客查看真实示例。

<http://chi.miantiao.me/post/cloudflare-worker-image/>

## 致谢

- [Cloudflare](https://www.cloudflare.com)
- [photon](https://github.com/silvia-odwyer/photon)
- [jSquash](https://github.com/jamsinclair/jSquash)

---

[![Buy Me A Coffee](https://static.miantiao.me/share/0WmsVP/CcmGr8.png)](https://www.buymeacoffee.com/ccbikai)
