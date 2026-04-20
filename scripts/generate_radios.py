#!/usr/bin/env python3
import asyncio
import json
import os
import re
import time
from pathlib import Path

import aiohttp
from aiohttp.resolver import AsyncResolver

CACHE_FILE = "cache.json"
CACHE_TTL = 86400

HEADERS = {"User-Agent": "Mozilla/5.0"}

TIMEOUT = aiohttp.ClientTimeout(total=8)

cpu_count = os.cpu_count() or 4
CONCURRENT_CHECKS = min(40, cpu_count)

GENRES = [
("60s", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/60s.m3u"),
    ("70s", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/70s.m3u"),
    ("80s", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/80s.m3u"),
    ("90s", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/90s.m3u"),
    ("Ambient", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/ambient.m3u"),
    ("Anime", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/anime.m3u"),
    ("Classical", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/classical.m3u"),
    ("Country", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/country.m3u"),
    ("Dance", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/dance.m3u"),
    ("Electronic", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/electronic.m3u"),
    ("Jazz", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/jazz.m3u"),
    ("Lo-Fi", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/chillout.m3u"),
    ("Rock", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/rock.m3u"),
    ("Japan", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/stream_finder/Japan.m3u"),
    ("K-Pop", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/stream_finder/K-Pop.m3u"),
    ("J-Pop", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/stream_finder/Jpop.m3u"),
    ("World", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/stream_finder/World.m3u"),
    ("DJ", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/streema/DJ.m3u"),
    ("India", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/streema/India.m3u"),
    ("Minimal", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/streema/Minimal.m3u"),
    ("Hindi", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/top-radio/hindi.m3u"),
    ("Hip-Hop", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/top-radio/hip-hop.m3u"),
    ("Metal", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/top-radio/metal.m3u"),
    ("Pop", "https://raw.githubusercontent.com/junguler/m3u-radio-music-playlists/main/top-radio/pop-music.m3u"),
]

def load_cache():
    if not Path(CACHE_FILE).exists():
        return {}
    try:
        return json.loads(Path(CACHE_FILE).read_text())
    except:
        return {}

def save_cache(cache):
    Path(CACHE_FILE).write_text(json.dumps(cache))

def get_cached(cache, url):
    e = cache.get(url)
    if not e:
        return None
    if time.time() - e.get("time", 0) > CACHE_TTL:
        return None
    return e

async def fetch_text(session, url):
    try:
        async with session.get(url) as r:
            if r.status == 200:
                return await r.text()
    except:
        return None

def parse_m3u(content, genre):
    out = []
    name = None
    for line in content.splitlines():
        line = line.strip()
        if line.startswith("#EXTINF"):
            m = re.search(r",(.+)$", line)
            if m:
                name = m.group(1)
        elif line and not line.startswith("#") and name:
            out.append((name.strip(), line.strip(), "Global", genre))
            name = None
    return out

def normalize_name(n):
    n = n.lower()
    n = re.sub(r"(radio|fm|am|hd|stream|live|music)", "", n)
    return re.sub(r"[^a-z0-9]", "", n)

def dedupe(stations):
    seen_url = set()
    seen_name = {}
    res = []
    for s in stations:
        name, url, *_ = s
        clean = normalize_name(name)
        if url in seen_url:
            continue
        if clean in seen_name:
            if len(url) >= len(seen_name[clean][1]):
                continue
            res.remove(seen_name[clean])
        seen_url.add(url)
        seen_name[clean] = s
        res.append(s)
    return res

def extract_meta(headers, chunk):
    ct = headers.get("Content-Type", "").lower()
    codec = "unknown"
    if "aac" in ct:
        codec = "aac"
    elif "mpeg" in ct or b"ICY" in chunk[:16]:
        codec = "mp3"
    br = headers.get("icy-br") or "?"
    return codec, br

def score(codec, br):
    s = 0
    if codec == "aac":
        s += 3
    elif codec == "mp3":
        s += 2
    try:
        s += min(int(br) // 64, 4)
    except:
        pass
    return s

def safe_score(meta):
    try:
        codec, br = meta.split(", ")[-2:]
        return score(codec, br)
    except:
        return 0

async def probe(session, url):
    for _ in range(2):
        try:
            async with session.get(url, timeout=TIMEOUT) as r:
                if r.status >= 400:
                    return None
                if "text/html" in r.headers.get("Content-Type", ""):
                    return None
                chunk = await r.content.read(2048)
                if not chunk:
                    return None
                return r.headers, chunk
        except aiohttp.ClientConnectorError:
            await asyncio.sleep(0.3)
        except:
            return None
    return None

async def check(session, sem, cache, s):
    name, url, country, tag = s

    c = get_cached(cache, url)
    if c:
        if c.get("alive"):
            return (name, url, country, f"{tag}, {c.get('codec','unknown')}, {c.get('bitrate','?')}kbps")
        return None

    async with sem:
        p = await probe(session, url)
        if not p:
            cache[url] = {"alive": False, "time": time.time()}
            return None

        headers, chunk = p
        codec, br = extract_meta(headers, chunk)

        cache[url] = {
            "alive": True,
            "time": time.time(),
            "codec": codec,
            "bitrate": br,
        }

        return (name, url, country, f"{tag}, {codec}, {br}kbps")

def write(stations):
    Path("src/core").mkdir(parents=True, exist_ok=True)
    with open("src/core/radio_stations.rs", "w") as f:
        f.write("pub const DEFAULT_RADIOS: &[(&str, &str, &str, &str)] = &[\n")
        for s in stations:
            f.write(f'''("{s[0].replace('"','\\"')}", "{s[1]}", "{s[2]}", "{s[3]}"),\n''')
        f.write("];\n")

async def main():
    start = time.time()
    cache = load_cache()

    resolver = AsyncResolver(nameservers=["1.1.1.1", "8.8.8.8"])

    conn = aiohttp.TCPConnector(
        limit=CONCURRENT_CHECKS,
        ttl_dns_cache=300,
        resolver=resolver,
    )

    async with aiohttp.ClientSession(headers=HEADERS, connector=conn) as session:
        contents = await asyncio.gather(*[fetch_text(session, u) for _, u in GENRES])

        all_s = []
        for (g, _), c in zip(GENRES, contents):
            if c:
                all_s.extend(parse_m3u(c, g))

        unique = dedupe(all_s)

        sem = asyncio.Semaphore(CONCURRENT_CHECKS)
        tasks = [check(session, sem, cache, s) for s in unique]

        valid = []
        for t in asyncio.as_completed(tasks):
            r = await t
            if r:
                valid.append(r)

        valid.sort(key=lambda x: safe_score(x[3]), reverse=True)

        save_cache(cache)
        write(valid)

    print(f"{len(valid)} active | {time.time()-start:.2f}s")

if __name__ == "__main__":
    asyncio.run(main())
