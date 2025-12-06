#!/usr/bin/env python3
import subprocess
import timeit
import sys

def run_benchmark(sg_cmd, py_path, description):
    # Run each benchmark 3 times and average
    sg_time = timeit.timeit(
        lambda: subprocess.run(sg_cmd, shell=True, capture_output=True), 
        number=3
    ) / 3
    
    py_time = timeit.timeit(
        lambda: subprocess.run(['python3', py_path], capture_output=True), 
        number=3
    ) / 3
    
    ratio = sg_time / py_time if py_time > 0 else 0
    
    if ratio < 1:
        perf = f"Stargate {1/ratio:.1f}x faster than Python"
    else:
        perf = f"Python {ratio:.1f}x faster than Stargate"
    
    return f"│ {description:23} │ {perf:36} │"

if __name__ == "__main__":
    if len(sys.argv) != 5:
        print("Usage: benchmark.py <sg_shell> <base_dir> <small_script> <large_script>")
        sys.exit(1)
    
    sg_shell = sys.argv[1]
    base_dir = sys.argv[2]
    small_sg = f"{base_dir}/stargate-language/benchmark/quicksort/benchmark_small.sg"
    small_py = f"{base_dir}/stargate-language/benchmark/quicksort/benchmark_small.py"
    large_sg = f"{base_dir}/stargate-language/benchmark/quicksort/benchmark_large.sg"
    large_py = f"{base_dir}/stargate-language/benchmark/quicksort/benchmark_large.py"
    
    print("┌─────────────────────────┬──────────────────────────────────────┐")
    print("│ Dataset                 │ Performance                          │")
    print("├─────────────────────────┼──────────────────────────────────────┤")
    print(run_benchmark(f"{sg_shell} {small_sg}", small_py, "Small (20 elements)"))
    print(run_benchmark(f"{sg_shell} {large_sg}", large_py, "Large (1000 elements)"))
    print("└─────────────────────────┴──────────────────────────────────────┘")
