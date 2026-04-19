#!/usr/bin/env python3
import os
import re
import glob

SOURCE_DIR = "/home/drack/tmp/script/m3u-radio-music-playlists"

def parse_m3u(file_path):
    stations = []
    genre = os.path.basename(file_path).replace(".m3u", "").replace("_", " ").title()
    try:
        with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
            lines = f.readlines()
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
                    else:
                        stations.append((line, line, "Global", genre))
    except Exception as e:
        print(f"Error parsing {file_path}: {e}")
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
        ("KissFM", "http://online.kissfm.ua/KissFM", "Ukraine", "Trance"),
    ]
    
    for s in original_curated:
        all_stations.append(s)
        seen_urls.add(s[1])

    m3u_files = glob.glob(os.path.join(SOURCE_DIR, "*.m3u"))
    m3u_files = [f for f in m3u_files if not os.path.basename(f).startswith("---")]
    m3u_files.sort()

    for file_path in m3u_files:
        file_stations = parse_m3u(file_path)
        for s in file_stations:
            if s[1] not in seen_urls:
                all_stations.append(s)
                seen_urls.add(s[1])

    print(f"Total unique stations found: {len(all_stations)}")

    with open("src/core/radio_stations.rs", "w") as f:
        f.write("pub const DEFAULT_RADIOS: &[(&str, &str, &str, &str)] = &[\n")
        for name, url, country, tags in all_stations:
            # Correct escaping order: backslash THEN quotes
            clean_name = name.replace('\\', '\\\\').replace('"', '\\"')
            clean_url = url.replace('\\', '\\\\').replace('"', '\\"')
            f.write(f'    (\n        "{clean_name}",\n        "{clean_url}",\n        "{country}",\n        "{tags}",\n    ),\n')
        f.write("];\n")

if __name__ == "__main__":
    main()
