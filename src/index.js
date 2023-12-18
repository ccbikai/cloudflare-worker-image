import queryString from 'query-string';

import * as photon from '@silvia-odwyer/photon';
import PHOTON_WASM from '../node_modules/@silvia-odwyer/photon/photon_rs_bg.wasm';

import encodeWebp, { init as initWebpWasm } from '@jsquash/webp/encode';
import WEBP_ENC_WASM from '../node_modules/@jsquash/webp/codec/enc/webp_enc.wasm';

// 图片处理
const photonInstance = await WebAssembly.instantiate(PHOTON_WASM, {
	'./photon_rs_bg.js': photon,
});
photon.setWasm(photonInstance.exports); // need patch

await initWebpWasm(WEBP_ENC_WASM);

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
	const [action, options = ''] = pipeAction.split('!');
	const params = options.split(',');
	if (multipleImageMode.includes(action)) {
		const image2 = params.shift(); // 是否需要 decodeURIComponent ?
		if (image2 && inWhiteList(env, image2)) {
			const image2Res = await fetch(image2, { headers: request.headers });
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
			console.log('cache: true');
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
					location: 'https://github.com/ccbikai/cloudflare-worker-image',
				},
			});
		}

		// 白名单检查
		if (!inWhiteList(env, url)) {
			console.log('whitelist: false');
			return new Response(null, {
				status: 403,
			});
		}

		// 目标图片获取与检查
		const imageRes = await fetch(url, { headers: request.headers });
		if (!imageRes.ok) {
			return imageRes;
		}
		console.log('fetch image done');

		const imageBytes = new Uint8Array(await imageRes.arrayBuffer());
		try {
			const inputImage = photon.PhotonImage.new_from_byteslice(imageBytes);
			console.log('create inputImage done');

			/** pipe
			 * `resize!800,400,1|watermark!https%3A%2F%2Fmt.ci%2Flogo.png,10,10,10,10`
			 */
			const pipe = action.split('|');
			const outputImage = await pipe.filter(Boolean).reduce(async (result, pipeAction) => {
				result = await result;
				return (await processImage(env, request, result, pipeAction)) || result;
			}, inputImage);
			console.log('create outputImage done');

			// 图片编码
			let outputImageData;
			if (format === 'jpeg' || format === 'jpg') {
				outputImageData = outputImage.get_bytes_jpeg(quality)
			} else if (format === 'png') {
				outputImageData = outputImage.get_bytes()
			} else {
				outputImageData = await encodeWebp(outputImage.get_image_data(), { quality });
			}
			console.log('create outputImageData done');

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
			console.log('image free done');

			// 写入缓存
			context.waitUntil(cache.put(cacheKey, imageResponse.clone()));
			return imageResponse;
		} catch (error) {
			console.error('process:error', error.name, error.message, error);
			const errorResponse = new Response(imageBytes || null, {
				headers: imageRes.headers,
				status: 'RuntimeError' === error.name ? 415 : 500,
			});
			return errorResponse;
		}
	},
};
