#!/usr/bin/env python3
"""Repeat controlled renders; report median transport time, never estimated GPU speedups."""
import argparse
import json
import platform
import statistics
import subprocess
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--backend', choices=['cpu', 'metal', 'both'], default='cpu')
parser.add_argument('--repeats', type=int, default=3)
parser.add_argument('--width', type=int, default=640)
parser.add_argument('--height', type=int, default=426)
parser.add_argument('--samples', type=int, default=64)
args = parser.parse_args()
if args.repeats < 1:
    parser.error('--repeats must be positive')
results = []
backends = ['cpu', 'metal'] if args.backend == 'both' else [args.backend]
with tempfile.TemporaryDirectory(prefix='lumen-benchmark-') as directory:
    for backend in backends:
        for scene in ['studio', 'interior', 'outdoor']:
            timings = []
            for repeat in range(args.repeats):
                output = Path(directory) / f'{backend}-{scene}-{repeat}.png'
                subprocess.run([str(root / 'target/release/lumen-rs'), '--backend', backend, '--scene', scene,
                                '--width', str(args.width), '--height', str(args.height), '--samples', str(args.samples),
                                '--seed', '42', '--output', str(output)], cwd=root, check=True)
                report = json.loads(output.with_suffix('.json').read_text())
                timings.append(report['render_seconds'])
            results.append({'backend': backend, 'scene': scene, 'median_seconds': statistics.median(timings),
                            'runs_seconds': timings, 'device': report['device'], 'primitives': report['primitives']})
summary = {'machine': platform.machine(), 'settings': vars(args), 'results': results}
(root / 'renders').mkdir(exist_ok=True)
(root / 'renders/benchmark.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
