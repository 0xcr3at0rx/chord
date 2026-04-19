#!/usr/bin/env python3
import requests
import re

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
]

def parse_m3u(genre, url):
    stations = []
    try:
        content = requests.get(url, timeout=10).text
        lines = content.splitlines()
        current_name = None
        for line in lines:
            line = line.strip()
            if not line: continue
            if line.startswith("#EXTINF"):
                match = re.search(r',(.+)$', line)
                if match:
                    current_name = match.group(1).strip()
            elif not line.startswith("#"):
                if current_name:
                    stations.append((current_name, line, "Global", genre))
                    current_name = None
    except Exception as e:
        print(f"Error parsing {genre}: {e}")
    return stations

def main():
    all_stations = []
    seen_urls = set()

    original_curated = [
        ("SomaFM Groove Salad", "http://ice1.somafm.com/groovesalad-128-mp3", "USA", "Ambient, Chillout"),
        ("SomaFM Drone Zone", "http://ice1.somafm.com/dronezone-128-mp3", "USA", "Ambient, Space"),
        ("FIP", "https://stream.radiofrance.fr/fip/fip_hifi.m3u8?id=radiofrance", "France", "Eclectic"),
        ("Radio Paradise (Main)", "http://stream.radioparadise.com/mp3-128", "USA", "Rock, Eclectic"),
        ("KEXP", "https://kexp-mp3-128.streamguys1.com/kexp128.mp3", "USA", "Alternative, Indie"),
        ("Jazz24", "https://live.jazz24.org/jazz24-mp3", "USA", "Jazz"),
        ("Cinemix", "http://94.23.252.14:8067/live", "France", "Soundtrack"),
        ("KissFM", "http://online.kissfm.ua/KissFM", "Ukraine", "Electronic, Trance"),
    ]
    
    for s in original_curated:
        all_stations.append(s)
        seen_urls.add(s[1])

    for genre, url in GENRES:
        print(f"Fetching {genre}...")
        genre_stations = parse_m3u(genre, url)
        for s in genre_stations:
            if s[1] not in seen_urls:
                all_stations.append(s)
                seen_urls.add(s[1])

    print(f"Total stations found: {len(all_stations)}")

    with open("src/core/radio_stations.rs", "w") as f:
        f.write("pub const DEFAULT_RADIOS: &[(&str, &str, &str, &str)] = &[\n")
        for name, url, country, tags in all_stations:
            clean_name = name.replace('"', '\\"').replace('\\', '\\\\')
            clean_url = url.replace('"', '\\"').replace('\\', '\\\\')
            f.write(f'    (\n        "{clean_name}",\n        "{clean_url}",\n        "{country}",\n        "{tags}",\n    ),\n')
        f.write("];\n")

if __name__ == "__main__":
    main()
