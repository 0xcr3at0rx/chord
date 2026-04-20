#!/usr/bin/env python3
import os
import re
import glob
import concurrent.futures
import urllib.request
import urllib.error

SOURCE_DIR = ""
OUTPUT_FILE = "radio.toml"
# Set to True to verify if streams are alive (slow for large lists)
CHECK_LIVENESS = False 
MAX_WORKERS = 20

# Filter keywords based on your request
ALLOWED_KEYWORDS = [
    "english", "usa", "uk", "canada", "australia", "england", "london", "new york",
    "japan", "japanese", "jp", "jpop", "j-pop", "anime", "tokyo",
    "korea", "korean", "kr", "kpop", "k-pop", "seoul",
    "china", "chinese", "cn", "cpop", "c-pop", "beijing", "shanghai",
    "hindi", "india", "indian", "bollywood", "punjabi", "alas"
]

def matches_filter(text):
    text = text.lower()
    return any(kw in text for kw in ALLOWED_KEYWORDS)

def parse_m3u(file_path, root_dir):
    stations = []
    rel_path = os.path.relpath(file_path, root_dir)
    genre = os.path.dirname(rel_path).replace(os.sep, " / ")
    filename = os.path.basename(file_path).replace(".m3u", "").replace("_", " ").title()
    
    if not genre:
        full_tags = filename
    else:
        full_tags = f"{genre} / {filename}"

    # Check if the category/file itself matches the filter
    file_matches = matches_filter(full_tags)

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
                    name = current_name if current_name else line
                    
                    # A station matches if its category matches OR its name matches
                    if file_matches or matches_filter(name):
                        stations.append({
                            "name": name,
                            "url": line,
                            "country": "Global",
                            "tags": full_tags
                        })
                    current_name = None
    except Exception as e:
        print(f"Error parsing {file_path}: {e}")
    return stations

def is_alive(station):
    url = station["url"]
    try:
        req = urllib.request.Request(url, headers={'User-Agent': 'Chord/1.0'})
        with urllib.request.urlopen(req, timeout=5) as response:
            if response.getcode() == 200:
                return station
    except:
        pass
    return None

def escape_toml(s):
    return s.replace('\\', '\\\\').replace('"', '\\"')

def validate_toml(file_path):
    print(f"Validating {file_path} syntax...")
    try:
        import tomllib
        with open(file_path, "rb") as f:
            tomllib.load(f)
        print("✓ TOML syntax is valid.")
        return True
    except ImportError:
        print("! Warning: Python version < 3.11, skipping built-in TOML validation.")
        return True
    except Exception as e:
        print(f"✗ TOML Syntax Error: {e}")
        return False

def main():
    if not os.path.exists(SOURCE_DIR):
        print(f"Error: Source directory {SOURCE_DIR} not found.")
        return

    print(f"Scanning {SOURCE_DIR} for filtered stations...")
    m3u_files = []
    for root, dirs, files in os.walk(SOURCE_DIR):
        for file in files:
            if file.endswith(".m3u") and not file.startswith("---"):
                m3u_files.append(os.path.join(root, file))
    
    m3u_files.sort()
    
    all_stations = []
    seen_urls = set()

    for file_path in m3u_files:
        stations = parse_m3u(file_path, SOURCE_DIR)
        for s in stations:
            if s["url"] not in seen_urls:
                all_stations.append(s)
                seen_urls.add(s["url"])

    print(f"Extracted {len(all_stations)} stations matching your filters.")

    valid_stations = all_stations
    if CHECK_LIVENESS and all_stations:
        print(f"Checking liveness with {MAX_WORKERS} workers...")
        valid_stations = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=MAX_WORKERS) as executor:
            future_to_station = {executor.submit(is_alive, s): s for s in all_stations}
            checked_count = 0
            for future in concurrent.futures.as_completed(future_to_station):
                res = future.result()
                if res:
                    valid_stations.append(res)
                checked_count += 1
                if checked_count % 100 == 0:
                    print(f"Progress: {checked_count}/{len(all_stations)} checked...")

    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        f.write("# Chord Radio Configuration\n")
        f.write(f"# Filtered for: English, Japan, Korea, China, India, Hindi\n\n")
        
        for s in valid_stations:
            f.write("[[stations]]\n")
            f.write(f'name = "{escape_toml(s["name"])}"\n')
            f.write(f'url = "{escape_toml(s["url"])}"\n')
            f.write(f'country = "{escape_toml(s["country"])}"\n')
            f.write(f'tags = "{escape_toml(s["tags"])}"\n\n')

    print(f"Created {OUTPUT_FILE} with {len(valid_stations)} filtered stations.")
    if valid_stations:
        validate_toml(OUTPUT_FILE)

if __name__ == "__main__":
    main()
