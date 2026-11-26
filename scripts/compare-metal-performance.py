#!/usr/bin/env python3
"""Compare two renderer binaries with identical paths and exact linear image checks."""
import argparse
import hashlib
import json
import platform
import statistics
import subprocess
from pathlib import Path


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', required=True, type=Path)
    parser.add_argument('--candidate', type=Path, default=root / 'target/release/lumen-rs')
    parser.add_argument('--scene', default='interior-night')
    parser.add_argument('--width', type=int, default=1920)
    parser.add_argument('--height', type=int, default=1080)
    parser.add_argument('--samples', type=int, default=16)
    parser.add_argument('--bounces', type=int, default=50)
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--output', type=Path, default=root / 'renders/performance/paired')
    args = parser.parse_args()
    if min(args.width, args.height, args.samples, args.bounces, args.repeats) < 1:
        parser.error('Dimensions, samples, bounces and repeats must be positive')
    binaries = {name: getattr(args, name).resolve() for name in ['baseline', 'candidate']}
    for binary in binaries.values():
        if not binary.is_file():
            parser.error(f'Binary not found: {binary}')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    runs = []
    # Alternate execution order to limit thermal/order bias. Run no other renders
    # or builds during this measurement; all timings come from completed renders.
    for repeat in range(args.repeats):
        reports = {}
        order = ['baseline', 'candidate'] if repeat % 2 == 0 else ['candidate', 'baseline']
        for name in order:
            stem = output / f'{name}-{repeat}'
            command = [str(binaries[name]), '--backend', 'metal', '--scene', args.scene,
                       '--width', str(args.width), '--height', str(args.height),
                       '--samples', str(args.samples), '--bounces', str(args.bounces),
                       '--seed', '42', '--adaptive', '0', '--exr', '--output', str(stem.with_suffix('.png'))]
            print(f'Run {repeat + 1}/{args.repeats}: {name}', flush=True)
            with stem.with_suffix('.log').open('w') as log:
                subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True)
            reports[name] = json.loads(stem.with_suffix('.json').read_text())
        for key in ['width', 'height', 'requested_spp', 'average_spp', 'max_depth', 'seed',
                    'adaptive_threshold', 'primitives', 'bvh_nodes', 'texture_count', 'scene_data_bytes']:
            if reports['baseline'][key] != reports['candidate'][key]:
                raise RuntimeError(f'Render settings or scene changed: {key}')
        if any(report['invalid_samples'] != 0 for report in reports.values()):
            raise RuntimeError('Invalid radiance samples detected')
        if any(report['average_spp'] != args.samples for report in reports.values()):
            raise RuntimeError('A fixed-sample render did not complete every requested sample')
        comparison_file = output / f'comparison-{repeat}.json'
        subprocess.run([str(root / 'target/release/compare'),
                        str(output / f'baseline-{repeat}.exr'),
                        str(output / f'candidate-{repeat}.exr'), '--output', str(comparison_file)],
                       cwd=root, check=True, stdout=subprocess.DEVNULL)
        comparison = json.loads(comparison_file.read_text())
        if comparison['maximum_absolute_error'] != 0:
            raise RuntimeError(f'Linear pixel mismatch: {comparison_file}')
        for key in ['reference', 'candidate']:
            location = Path(comparison[key])
            comparison[key] = str(location.relative_to(root)) if location.is_relative_to(root) else location.name
        runs.append({'order': order, **reports, 'comparison': comparison})
        print(f"  {reports['baseline']['render_seconds']:.3f} -> "
              f"{reports['candidate']['render_seconds']:.3f} s; exact pixels", flush=True)
    medians = {name: statistics.median(run[name]['render_seconds'] for run in runs)
               for name in binaries}
    summary = {
        'machine': platform.machine(), 'scene': args.scene,
        'settings': {key: getattr(args, key) for key in ['width', 'height', 'samples', 'bounces', 'repeats']},
        'binary_sha256': {name: hashlib.sha256(binary.read_bytes()).hexdigest()
                          for name, binary in binaries.items()},
        'median_seconds': medians,
        'speedup': medians['baseline'] / medians['candidate'],
        'time_reduction_percent': 100 * (1 - medians['candidate'] / medians['baseline']),
        'note': 'Measured transport only; scene loading, shader compilation and denoising excluded. '
                'No prediction of a completed 2048-spp render. All raw linear pixels match exactly.',
        'runs': runs,
    }
    (output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps({key: value for key, value in summary.items() if key != 'runs'}, indent=2))


if __name__ == '__main__':
    main()
