import requests
import concurrent.futures

# Default radios from src/core/radio_stations.rs
DEFAULT_RADIOS = [
    ("SomaFM Groove Salad", "http://ice1.somafm.com/groovesalad-128-mp3"),
    ("SomaFM Drone Zone", "http://ice1.somafm.com/dronezone-128-mp3"),
    ("FIP", "https://stream.radiofrance.fr/fip/fip_hifi.m3u8?id=radiofrance"),
    ("Radio Paradise (Main)", "http://stream.radioparadise.com/mp3-128"),
    ("KEXP", "https://kexp-mp3-128.streamguys1.com/kexp128.mp3"),
    ("NTS Radio 1", "https://stream-relay-geo.ntslive.net/stream"),
    ("Antenne Bayern", "http://stream.antenne.de/antenne"),
    ("Jazz24", "https://live.jazz24.org/jazz24-mp3"),
    ("Cinemix", "http://94.23.252.14:8067/live"),
    ("Swiss Groove", "https://icecast.argon.ch/swissgroove"),
    ("Radio 105", "http://shoutcast.radio105.it:8000/105.mp3"),
    ("Classic FM", "http://stream.live.vc.bbc.co.uk/bbc_radio_three_offline"),
    ("Acid House", "http://abm22.com.au:8000/CONTAINER1"),
    ("Afro House", "http://abm22.com.au:8000/CONTAINER53"),
    ("KissFM", "http://online.kissfm.ua/KissFM"),
    ("Paddygrooves", "https://a12.siar.us/radio/8230/radio.mp3"),
    ("Funky Ass Tunes", "https://ams1.reliastream.com/proxy/john12/stream"),
    ("Abaco Libros y Cafe", "https://radio30.virtualtronics.com/proxy/abaco"),
    ("Blues Revue", "http://live.str3am.com:2240/live"),
    ("Jazz & Blues Radio", "https://jazzblues.ice.infomaniak.ch/jazzblues-high.mp3"),
    ("Jazz Eire", "https://visual.shoutca.st:8096/stream"),
    ("Art Bell Radio", "http://stream.willstare.com:8450/"),
    ("Roots FM", "http://138.201.198.218:8043/stream"),
    ("Double J", "http://live-radio01.mediahubaustralia.com/2jr/mp3/"),
    ("Svensk Folkmusik AkkA", "https://mediaserv38.live-streams.nl:8107/stream"),
    ("Ambient FM", "https://phoebe.streamerr.co:4140/ambient.mp3"),
    ("Chillsky Chillhop", "https://chill.radioca.st/stream"),
    ("Nature Radio", "https://nature-rex.radioca.st/stream"),
    ("FreeCodeCamp Radio", "https://stream.freecodecamp.org/radio.mp3"),
    ("Robert Loglisci Radio", "https://radio.loglisci.com/listen/robertloglisciradio/radio.mp3"),
    ("Cubic Space Radio", "http://music.cubicspace.fm:42424/mpeg"),
    ("Radio Regional Portugal", "http://193.70.40.92:8000/stream/2/"),
    ("Radio 100% Brasil", "http://193.70.40.92:8000/stream/11/"),
    ("Da Hub Radio", "https://stream.dahubradio.co.uk:8848/stream"),
    ("Voltaje Radio", "https://server5.mediasector.es:8070/voltaje"),
    ("La Diaria Radio", "https://radiolatina.live/8156/stream"),
    ("More Public Radio", "http://68.233.231.202:8107/stream"),
    ("Classic Hip Hop Radio", "http://73.191.71.239:8000/;"),
    ("Rock Steady 94 Country", "http://streamingcenter.radiohosting.live:1830/stream"),
]

def check_station(name, url):
    try:
        # Use a short timeout and stream=True to avoid downloading the whole stream
        response = requests.get(url, timeout=10, stream=True, headers={'User-Agent': 'Chord/HealthCheck'})
        if response.status_code == 200:
            return f"[OK] {name}"
        else:
            return f"[FAILED] {name} (Status: {response.status_code})"
    except Exception as e:
        return f"[ERROR] {name} ({str(e)})"

def main():
    print(f"Checking {len(DEFAULT_RADIOS)} radio stations...")
    with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
        futures = [executor.submit(check_station, name, url) for name, url in DEFAULT_RADIOS]
        for future in concurrent.futures.as_completed(futures):
            print(future.result())

if __name__ == "__main__":
    main()
