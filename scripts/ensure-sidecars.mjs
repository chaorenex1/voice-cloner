#!/usr/bin/env node
import { createWriteStream, existsSync, mkdirSync, readdirSync, statSync, chmodSync, copyFileSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import https from 'node:https';
import http from 'node:http';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..');
const binariesDir = join(repoRoot, 'src-tauri', 'binaries');
const cacheDir = join(repoRoot, 'src-tauri', '.sidecar-cache');
const force = process.argv.includes('--force') || process.env.VOICE_CLONER_FORCE_SIDECAR_DOWNLOAD === '1';
const checkOnly = process.argv.includes('--check');
const dryRun = process.argv.includes('--dry-run');
const skip = process.env.VOICE_CLONER_SKIP_SIDECAR_DOWNLOAD === '1';

const target = detectTarget();
const specs = sidecarSpecs(target);

if (skip) {
  console.log('[sidecars] skipped by VOICE_CLONER_SKIP_SIDECAR_DOWNLOAD=1');
  process.exit(0);
}

mkdirSync(binariesDir, { recursive: true });
mkdirSync(cacheDir, { recursive: true });

for (const spec of specs) {
  await ensureSidecar(spec);
}

async function ensureSidecar(spec) {
  if (isUsableFile(spec.targetPath) && !force) {
    console.log(`[sidecars] ${spec.name} already available: ${relative(spec.targetPath)}`);
    return;
  }

  if (checkOnly || dryRun) {
    const mode = checkOnly ? 'check' : 'dry-run';
    console.log(`[sidecars] ${mode}: would download ${spec.name} from ${spec.url}`);
    console.log(`[sidecars] ${mode}: would install to ${relative(spec.targetPath)}`);
    return;
  }

  const archivePath = join(cacheDir, spec.archiveName);
  const extractDir = join(cacheDir, `${spec.name}-${target.triple}`);
  rmSync(extractDir, { recursive: true, force: true });
  mkdirSync(extractDir, { recursive: true });

  console.log(`[sidecars] downloading ${spec.name}: ${spec.url}`);
  await download(spec.url, archivePath);
  console.log(`[sidecars] extracting ${spec.archiveName}`);
  extractArchive(archivePath, extractDir);

  const executable = findExecutable(extractDir, spec.executableNames);
  if (!executable) {
    throw new Error(`Downloaded ${spec.name}, but no executable named ${spec.executableNames.join(', ')} was found`);
  }

  copyFileSync(executable, spec.targetPath);
  if (process.platform !== 'win32') {
    chmodSync(spec.targetPath, 0o755);
  }
  const size = statSync(spec.targetPath).size;
  if (size <= 0) {
    throw new Error(`Installed ${spec.name} is empty: ${spec.targetPath}`);
  }
  console.log(`[sidecars] installed ${spec.name} (${Math.round(size / 1024 / 1024)} MB): ${relative(spec.targetPath)}`);
}

function detectTarget() {
  const { platform, arch } = process;
  if (platform === 'win32' && arch === 'x64') {
    return { platform, arch, triple: 'x86_64-pc-windows-msvc', exe: '.exe' };
  }
  if (platform === 'linux' && arch === 'x64') {
    return { platform, arch, triple: 'x86_64-unknown-linux-gnu', exe: '' };
  }
  if (platform === 'darwin' && arch === 'arm64') {
    return { platform, arch, triple: 'aarch64-apple-darwin', exe: '' };
  }
  throw new Error(`Unsupported sidecar target: ${platform}/${arch}. demucs-rs releases currently cover Windows x64, Linux x64, and macOS arm64.`);
}

function sidecarSpecs(target) {
  const demucsVersion = process.env.VOICE_CLONER_DEMUCS_RS_VERSION || 'v0.3.4';
  const demucsAsset = target.platform === 'win32'
    ? `demucs-${target.triple}.zip`
    : `demucs-${target.triple}.tar.gz`;
  const ffmpegAsset = ffmpegAssetFor(target);
  return [
    {
      name: 'ffmpeg',
      archiveName: ffmpegAsset.name,
      url: ffmpegAsset.url,
      executableNames: target.platform === 'win32' ? ['ffmpeg.exe'] : ['ffmpeg'],
      targetPath: join(binariesDir, `ffmpeg-${target.triple}${target.exe}`),
    },
    {
      name: 'demucs-rs',
      archiveName: demucsAsset,
      url: `https://github.com/nikhilunni/demucs-rs/releases/download/${demucsVersion}/${demucsAsset}`,
      executableNames: target.platform === 'win32' ? ['demucs.exe', 'demucs-rs.exe'] : ['demucs', 'demucs-rs'],
      targetPath: join(binariesDir, `demucs-rs-${target.triple}${target.exe}`),
    },
  ];
}

function ffmpegAssetFor(target) {
  if (target.platform === 'win32') {
    return {
      name: 'ffmpeg-n8.1-latest-win64-lgpl-8.1.zip',
      url: 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-n8.1-latest-win64-lgpl-8.1.zip',
    };
  }
  if (target.platform === 'linux') {
    return {
      name: 'ffmpeg-n8.1-latest-linux64-lgpl-8.1.tar.xz',
      url: 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-n8.1-latest-linux64-lgpl-8.1.tar.xz',
    };
  }
  return {
    name: 'ffmpeg-evermeet-macos.zip',
    url: 'https://evermeet.cx/ffmpeg/getrelease/zip',
  };
}

function isUsableFile(path) {
  try {
    return statSync(path).isFile() && statSync(path).size > 0;
  } catch (_error) {
    return false;
  }
}

function download(url, targetPath, redirects = 0) {
  if (redirects > 5) {
    return Promise.reject(new Error(`Too many redirects while downloading ${url}`));
  }
  return new Promise((resolvePromise, reject) => {
    const client = url.startsWith('http://') ? http : https;
    const request = client.get(url, { headers: { 'User-Agent': 'voice-cloner-sidecar-downloader' } }, (response) => {
      const status = response.statusCode ?? 0;
      if ([301, 302, 303, 307, 308].includes(status) && response.headers.location) {
        response.resume();
        const nextUrl = new URL(response.headers.location, url).toString();
        download(nextUrl, targetPath, redirects + 1).then(resolvePromise, reject);
        return;
      }
      if (status < 200 || status >= 300) {
        response.resume();
        reject(new Error(`Download failed (${status}) for ${url}`));
        return;
      }
      const file = createWriteStream(targetPath);
      response.pipe(file);
      file.on('finish', () => file.close(resolvePromise));
      file.on('error', reject);
    });
    request.on('error', reject);
  });
}

function extractArchive(archivePath, outputDir) {
  if (archivePath.endsWith('.zip')) {
    if (process.platform === 'win32') {
      run('powershell', ['-NoProfile', '-Command', `Expand-Archive -LiteralPath ${quotePwsh(archivePath)} -DestinationPath ${quotePwsh(outputDir)} -Force`]);
      return;
    }
    run('unzip', ['-q', '-o', archivePath, '-d', outputDir]);
    return;
  }
  run('tar', ['-xf', archivePath, '-C', outputDir]);
}

function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(' ')} failed with exit code ${result.status}`);
  }
}

function findExecutable(root, names) {
  const entries = readdirSync(root, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      const found = findExecutable(path, names);
      if (found) return found;
    } else if (names.some((name) => entry.name.toLowerCase() === name.toLowerCase())) {
      return path;
    }
  }
  return null;
}

function quotePwsh(value) {
  return `'${value.replaceAll("'", "''")}'`;
}

function relative(path) {
  return path.replace(`${repoRoot}\\`, '').replace(`${repoRoot}/`, '');
}
