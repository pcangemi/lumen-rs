#!/usr/bin/env python3
"""Measure Lumen and Cycles separately against their own independent high-sample references."""
import argparse
import json
import platform
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def execute(command, log):
    started = time.perf_counter()
    print(f'Starting {log.stem}; log: {log}', flush=True)
    with log.open('w') as stream:
        process = subprocess.Popen([str(x) for x in command], cwd=ROOT,
                                   stdout=stream, stderr=subprocess.STDOUT)
        try:
            while True:
                try:
                    code = process.wait(timeout=30)
                    break
                except subprocess.TimeoutExpired:
                    print(f'{log.stem}: {time.perf_counter() - started:.0f} s elapsed', flush=True)
        except KeyboardInterrupt:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            raise
        if code:
            raise RuntimeError(f'{log.stem} failed with exit code {code}; inspect {log}')
    elapsed = time.perf_counter() - started
    print(f'Finished {log.stem}: {elapsed:.2f} s', flush=True)
    return elapsed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--blender', type=Path, default=Path('/Applications/Blender.app/Contents/MacOS/Blender'))
    parser.add_argument('--scene', choices=['studio', 'interior', 'outdoor'], default='studio')
    parser.add_argument('--backend', choices=['cpu', 'metal'], default='metal')
    parser.add_argument('--width', type=int, default=640)
    parser.add_argument('--height', type=int, default=426)
    parser.add_argument('--samples', type=int, default=128)
    parser.add_argument('--reference-samples', type=int, default=2048)
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--quick', action='store_true', help='Use 320 x 213, 32 candidate samples, 256 reference samples and one repeat')
    parser.add_argument('--output', type=Path, default=ROOT / 'renders/cycles-comparison')
    args = parser.parse_args()
    if args.quick:
        args.width, args.height, args.samples, args.reference_samples, args.repeats = 320, 213, 32, 256, 1
    if args.repeats < 1 or args.samples < 1 or args.reference_samples <= args.samples:
        parser.error('Require repeats >= 1 and reference-samples > samples > 0')
    if not args.blender.is_file():
        parser.error('Blender executable not found; use --blender /path/to/Blender')
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    print(f'Rendering {2 * (args.repeats + 1)} images: two {args.reference_samples}-sample references '
          f'and {2 * args.repeats} {args.samples}-sample candidates.', flush=True)
    print('The first Cycles Metal run may spend several minutes compiling shaders. '
          'Use --quick for workflow validation.', flush=True)
    snapshot = args.output / 'snapshot'
    renderer = ROOT / 'target/release/lumen-rs'
    common = [renderer, '--scene', args.scene, '--width', args.width, '--height', args.height,
              '--bounces', 12, '--backend', args.backend]
    execute([*common, '--export-comparison', snapshot], args.output / 'export.log')
    reports = []
    for engine in ['lumen', 'cycles']:
        timings = []
        reference = args.output / f'{engine}-reference.exr'
        for run in range(args.repeats + 1):
            is_reference = run == 0
            stem = f'{engine}-reference' if is_reference else f'{engine}-{run}'
            samples = args.reference_samples if is_reference else args.samples
            # An independent reference seed avoids spuriously low error from shared sample prefixes.
            seed = 1701 if is_reference else 42 + run
            image = args.output / f'{stem}.exr'
            if engine == 'lumen':
                command = [*common, '--samples', samples, '--seed', seed,
                           '--output', image.with_suffix('.png'), '--exr']
            else:
                command = [args.blender, '--background', '--factory-startup', '--python-exit-code', 1,
                           '--python', ROOT / 'scripts/cycles_scene.py', '--',
                           '--snapshot', snapshot / 'scene.json', '--backend', args.backend,
                           '--samples', samples, '--seed', seed, '--output', image]
            elapsed = execute(command, args.output / f'{stem}.log')
            report = json.loads(image.with_suffix('.json').read_text())
            report.update(engine=engine, process_wall_seconds=elapsed, reference=is_reference)
            if not is_reference:
                timings.append(elapsed)
                metric = args.output / f'{stem}-error.json'
                execute([ROOT / 'target/release/compare', reference, image, '--output', metric],
                        args.output / f'{stem}-error.log')
                report['error'] = json.loads(metric.read_text())
            reports.append(report)
        print(f'{engine}: median process wall time {statistics.median(timings):.3f} s', flush=True)
    revision = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT, capture_output=True, text=True)
    result = {'revision': revision.stdout.strip() or None, 'platform': platform.platform(), 'settings': {k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
              'runs': reports, 'comparison_status': 'Raw convergence measurements; inspect shader mapping and reference noise before ranking performance.',
              'timing_scope': 'Process wall time includes startup, scene construction, acceleration structure build, rendering and image output. Export preparation excluded.'}
    (args.output / 'comparison.json').write_text(json.dumps(result, indent=2) + '\n')
    print(args.output / 'comparison.json')


if __name__ == '__main__':
    main()
