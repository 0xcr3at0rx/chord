#!/usr/bin/env python3
import asyncio
import json
import os
import re
import time
from pathlib import Path

import aiohttp

CACHE_FILE = "cache.json"
CACHE_TTL = 86400

HEADERS = {"User-Agent": "Mozilla/5.0"}

TIMEOUT = aiohttp.ClientTimeout(total=5)

CONCURRENT = min(100, (os.cpu_count() or 4) * 10)

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
    if Path(CACHE_FILE).exists():
        return json.loads(Path(CACHE_FILE).read_text())
    return {}

def save_cache(c):
    Path(CACHE_FILE).write_text(json.dumps(c))

def cached_ok(cache, url):
    c = cache.get(url)
    if c and time.time() - c["time"] < CACHE_TTL:
        return c
    return None

async def fetch(session, url):
    try:
        async with session.get(url) as r:
            if r.status == 200:
                return await r.text()
    except:
        return None

def parse_m3u(txt, genre):
    out, name = [], None
    for line in txt.splitlines():
        line = line.strip()
        if line.startswith("#EXTINF"):
            name = line.split(",", 1)[-1]
        elif line and not line.startswith("#") and name:
            out.append((name, line, genre))
            name = None
    return out

def norm(name):
    name = name.lower()
    name = re.sub(r"(radio|fm|am|hd|stream|live)", "", name)
    return re.sub(r"[^a-z0-9]", "", name)

def dedupe(stations):
    best = {}

    for name, url, genre in stations:
        key = norm(name)

        if key not in best:
            best[key] = (name, url, genre, 0)
            continue

        # keep shorter URL (usually cleaner stream)
        if len(url) < len(best[key][1]):
            best[key] = (name, url, genre, 0)

    return [(v[0], v[1], v[2]) for v in best.values()]

def codec_score(ct):
    ct = ct.lower()
    if "aac" in ct:
        return "aac", 3
    if "mpeg" in ct:
        return "mp3", 2
    return "?", 0

async def check(session, sem, cache, st):
    name, url, genre = st

    c = cached_ok(cache, url)
    if c:
        return (name, url, genre, c["meta"]) if c["alive"] else None

    async with sem:
        try:
            async with session.head(url, allow_redirects=True) as r:
                if r.status >= 400:
                    raise Exception()

                ct = r.headers.get("Content-Type", "")
                if "text/html" in ct:
                    raise Exception()

                codec, score = codec_score(ct)
                br = r.headers.get("icy-br", "?")

                meta = f"{genre}, {codec}, {br}kbps"

                cache[url] = {"alive": True, "meta": meta, "time": time.time()}

                return (name, url, genre, meta)

        except:
            cache[url] = {"alive": False, "time": time.time()}
            return None

def score(meta):
    parts = meta.split(", ")
    codec = parts[1]
    br = parts[2].replace("kbps", "")

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

def write(stations):
    Path("src/core").mkdir(parents=True, exist_ok=True)

    with open("src/core/radio_stations.rs", "w") as f:
        f.write("pub const DEFAULT_RADIOS: &[(&str,&str,&str,&str)] = &[\n")
        for s in stations:
            f.write(f'("{s[0]}","{s[1]}","Global","{s[3]}"),\n')
        f.write("];")

async def main():
    start = time.time()
    cache = load_cache()

    async with aiohttp.ClientSession(headers=HEADERS, timeout=TIMEOUT) as session:
        texts = await asyncio.gather(*[fetch(session, u) for _, u in GENRES])

        all_stations = []
        for (g, _), t in zip(GENRES, texts):
            if t:
                all_stations += parse_m3u(t, g)

        print("Raw:", len(all_stations))

        unique = dedupe(all_stations)
        print("Unique:", len(unique))

        sem = asyncio.Semaphore(CONCURRENT)

        tasks = [check(session, sem, cache, s) for s in unique]

        valid = []
        for i, coro in enumerate(asyncio.as_completed(tasks), 1):
            r = await coro
            if r:
                valid.append(r)

            if i % 50 == 0:
                print(f"{i}/{len(unique)}")

        print("Active:", len(valid))

        valid.sort(key=lambda x: score(x[3]), reverse=True)

        save_cache(cache)
        write(valid)

    print("Done:", round(time.time() - start, 2), "s")


if __name__ == "__main__":
    asyncio.run(main())
