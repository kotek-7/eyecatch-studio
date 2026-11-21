"""Restore missing assets from the pinned URLs in catalog.json; keep existing files."""
import concurrent.futures
import json
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
catalog = json.loads((ROOT / 'catalog.json').read_text())
sources = json.loads((ROOT / 'assets/sources.json').read_text())
jobs = {p['asset']: p['source'] for p in catalog if p['source'].startswith('https://')}
for repo, filename in [('devicons/devicon', 'DEVICON-LICENSE'), ('lucide-icons/lucide', 'LUCIDE-LICENSE')]:
    jobs[f'assets/{filename}'] = f'https://raw.githubusercontent.com/{repo}/{sources[repo]}/LICENSE'


def download(job):
    name, url = job
    target = (ROOT / name).resolve()
    if not target.is_relative_to(ROOT / 'assets'):
        raise ValueError(f'Asset outside assets/: {name}')
    if target.exists():
        return
    request = urllib.request.Request(url, headers={'User-Agent': 'eyecatch-studio'})
    with urllib.request.urlopen(request, timeout=45) as response:
        data = response.read()
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)
    print(f'Downloaded {name}')


with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
    list(pool.map(download, jobs.items()))
print(f'Checked {len(jobs)} source files for {len(catalog)} presets')
