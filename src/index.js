import queryString from 'query-string';

import * as photon from '@silvia-odwyer/photon';
import * as PHOTON_WASM_JS from '../node_modules/@silvia-odwyer/photon/photon_rs_bg.js';
import PHOTON_WASM from '../node_modules/@silvia-odwyer/photon/photon_rs_bg.wasm';

import encodeWebp, { init as initWebpWasm } from '@jsquash/webp/encode';
import encodeJpeg, { init as initJpegWasm } from '@jsquash/jpeg/encode';
import encodePng, { init as initPngWasm } from '@jsquash/png/encode';

import WEBP_ENC_WASM from '../node_modules/@jsquash/webp/codec/enc/webp_enc.wasm';
import JPEG_ENC_WASM from '../node_modules/@jsquash/jpeg/codec/enc/mozjpeg_enc.wasm';
import PNG_ENC_WASM from '../node_modules/@jsquash/png/codec/squoosh_png_bg.wasm';

const OUTPUT_FORMATS = {
	jpeg: 'image/jpeg',
	jpg: 'image/jpeg',
	png: 'image/png',
	webp: 'image/webp',
};

const multipleImageMode = ['watermark', 'blend'];

const inWhiteList = (env, url) => {
	const imageUrl = new URL(url);
	const whiteList = env.WHITE_LIST ? env.WHITE_LIST.split(',') : [];
	return !(whiteList.length && !whiteList.find((hostname) => imageUrl.hostname.endsWith(hostname)));
};

const processImage = async (env, request, inputImage, pipeAction) => {
	const [ action, options = '' ] = pipeAction.split('!')
	const params = options.split(',');
	if (multipleImageMode.includes(action)) {
		const image2 = params.shift(); // 是否需要 decodeURIComponent ?
		if (image2 && inWhiteList(env, image2)) {
			const image2Res = await fetch(image2, {headers: request.headers});
			if (image2Res.ok) {
				const inputImage2 = photon.PhotonImage.new_from_byteslice(new Uint8Array(await image2Res.arrayBuffer()));
				// 多图处理是处理原图
				photon[action](inputImage, inputImage2, ...params);
				return inputImage; // 多图模式返回第一张图
			}
		}
	} else {
		return photon[action](inputImage, ...params);
	}
};

export default {
	async fetch(request, env, context) {
		// 读取缓存
		const cacheUrl = new URL(request.url);
		const cacheKey = new Request(cacheUrl.toString());
		const cache = caches.default;
		const hasCache = await cache.match(cacheKey);
		if (hasCache) {
			return hasCache;
		}

		// 入参提取与校验
		const query = queryString.parse(new URL(request.url).search);
		const { url = '', action = '', format = 'webp', quality = 99 } = query;
		console.log('params:', url, action, format, quality);

		if (!url) {
			return new Response(null, {
				status: 302,
				headers: {
					location: 'https://github.com/ccbikai/cloudflare-worker-image'
				}
			});
		}

		// 白名单检查
		if (!inWhiteList(env, url)) {
			return new Response(null, {
				status: 403,
			});
		}

		// 目标图片获取与检查
		const imageRes = await fetch(url, {headers: request.headers});
		if (!imageRes.ok) {
			return imageRes;
		}

		const imageBytes = new Uint8Array(await imageRes.arrayBuffer());
		try {
			// 图片处理
			const photonInstance = new WebAssembly.Instance(PHOTON_WASM, {
				'./photon_rs_bg.js': PHOTON_WASM_JS,
			});
			photon.setWasm(photonInstance.exports); // need patch
			const inputImage = photon.PhotonImage.new_from_byteslice(imageBytes);

			/** pipe
				 * `resize!800,400,1|watermark!https%3A%2F%2Fmt.ci%2Flogo.png,10,10,10,10`
				 */
			const pipe = action.split('|')
			const outputImage = await pipe.reduce(async (result, pipeAction) => {
				result = await result;
				return (await processImage(env, request, result, pipeAction)) || result;
			}, inputImage);

			// 图片编码
			let outputImageData;
			if (format === 'jpeg' || format === 'jpg') {
				await initJpegWasm(JPEG_ENC_WASM);
				outputImageData = await encodeJpeg(outputImage.get_image_data(), { quality });
			} else if (format === 'png') {
				await initPngWasm(PNG_ENC_WASM);
				outputImageData = await encodePng(outputImage.get_image_data());
			} else {
				await initWebpWasm(WEBP_ENC_WASM);
				outputImageData = await encodeWebp(outputImage.get_image_data(), { quality });
			}

			// 返回体构造
			const imageResponse = new Response(outputImageData, {
				headers: {
					'content-type': OUTPUT_FORMATS[format],
					'cache-control': 'public,max-age=15552000',
				},
			});

			// 释放资源
			inputImage.ptr && inputImage.free();
			outputImage.ptr && outputImage.free();

			// 写入缓存
			context.waitUntil(cache.put(cacheKey, imageResponse.clone()));
			return imageResponse;
		} catch (error) {
			console.error('process:error', error.name, error.message, error);
			const errorResponse = new Response(imageBytes, {
				headers: imageRes.headers,
				status: 'RuntimeError' === error.name ? 415 : 500,
			});
			return errorResponse;
		}
	},
};
