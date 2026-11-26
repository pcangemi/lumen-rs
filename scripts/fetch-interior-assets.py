"""Restore pinned model sources before running prepare-interior-assets.py in Blender."""
import hashlib
import json
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    records = json.loads((ROOT / 'assets/models/sources.json').read_text())
    manifest = []
    for record in records:
        for item in record['files']:
            relative = Path('.runtime/source-assets') / record['asset'] / item['path']
            path = ROOT / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists() or hashlib.md5(path.read_bytes()).hexdigest() != item['md5']:
                print(f"Downloading {record['asset']}/{item['path']}", flush=True)
                request = urllib.request.Request(item['url'], headers={'User-Agent': 'Lumen asset preparation'})
                with urllib.request.urlopen(request, timeout=120) as response:
                    data = response.read()
                if hashlib.md5(data).hexdigest() != item['md5']:
                    raise ValueError(f'Source checksum mismatch: {path}')
                path.write_bytes(data)
            manifest.append(dict(asset=record['asset'], file=str(relative), url=item['url'], md5=item['md5']))
    (ROOT / '.runtime/source-assets/manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')


if __name__ == '__main__':
    main()
