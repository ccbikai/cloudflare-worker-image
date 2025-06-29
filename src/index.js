import { Container, getRandom } from '@cloudflare/containers';

export class PhotonContainer extends Container {
	defaultPort = 8000;
	sleepAfter = '2m';

	onError(error, request) {
		console.log('Container error:', error, 'url:', request?.url);
	}
}

const inWhiteList = (env, url) => {
	const imageUrl = new URL(url);
	const whiteList = env.WHITE_LIST ? env.WHITE_LIST.split(',') : [];
	return !(whiteList.length && !whiteList.find((hostname) => imageUrl.hostname.endsWith(hostname)));
};

export default {
	async fetch(request, env, context) {
		try {
			// 读取缓存
			const cacheUrl = new URL(request.url);
			const cacheKey = new Request(cacheUrl.toString());
			const cache = caches.default;
			const cacheResponse = await cache.match(cacheKey);

			if (cacheResponse) {
				return cacheResponse;
			}

			const { pathname, searchParams } = new URL(request.url);
			const url = searchParams.get('url');

			if (!url && pathname === '/') {
				return Response.redirect('https://github.com/ccbikai/cloudflare-worker-image', 302);
			}

			// 白名单检查
			if (url && !inWhiteList(env, url)) {
				return new Response(null, {
					status: 403,
				});
			}

			const container = await getRandom(env.PHOTON_CONTAINER, 2);
			const imageResponse = await container.fetch(request);

			// 写入缓存
			context.waitUntil(cache.put(cacheKey, imageResponse.clone()));
			return imageResponse;
		} catch (error) {
			console.error('Failed to process request:', error, 'url:', request.url);
			return new Response('Failed to process request', { status: 500 });
		}
	},
}
