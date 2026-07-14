import type { HostClient } from './client';
import type { AppBuildDto } from './types';

let cachedBuild: Promise<AppBuildDto | null> | null = null;

export function loadAppBuild(host: Pick<HostClient, 'appBuild'>): Promise<AppBuildDto | null> {
	if (!cachedBuild) {
		cachedBuild = host.appBuild().catch(() => null);
	}
	return cachedBuild;
}

export function formatAppBuild(build: AppBuildDto): string {
	return `v${build.version} (${build.sha})`;
}
