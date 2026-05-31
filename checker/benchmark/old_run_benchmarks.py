import os
import subprocess
import re
import csv
from datetime import datetime
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent.parent
EXAMPLES_DIR = PROJECT_ROOT / "data/raw"
CHECKER_BIN = PROJECT_ROOT / "target" / "release" / "checker"
BENCHMARK_DIR = PROJECT_ROOT / "benchmark"

TIMEOUT_LIMIT = 60

INSTANCES = [
    "dining_8.smv",
    "dining_10.smv",
    "dining_12.smv",
    "dining_14.smv",
]

CHECKER_ALGORITHMS = ["bdd", "labelling-scc"]

NUSMV_TEMPLATE = """
set default_trace_plugin 0
set cone_of_influence
set bdd_static_order_heuristics none
set vars_order_type topological
read_model -i {model_path}
flatten_hierarchy
encode_variables
build_model -m Monolithic
echo "[MILESTONE 1]"
print_usage
check_ctlspec
echo "[MILESTONE 2]"
print_usage
quit
"""

def get_peak_mem(stderr_output):
    """Extracts Maximum resident set size from 'time -v' output."""
    match = re.search(r"Maximum resident set size \(kbytes\): (\d+)", stderr_output)
    return int(match.group(1)) if match else 0

def parse_nusmv_output(stdout_output):
    """Extracts timings and nodes from NuSMV's internal usage log."""
    user_times = re.findall(r"User time\s+(\d+\.\d+)\s+seconds", stdout_output)
    nodes_allocated = re.findall(r"BDD nodes allocated:\s+(\d+)", stdout_output)
    if len(user_times) >= 2 and len(nodes_allocated) >= 2:
        t1 = float(user_times[0]) * 1000
        t_total = float(user_times[-1]) * 1000
        return {
            "compile_ms": int(t1),
            "verify_ms": int(t_total - t1),
            "total_ms": int(t_total),
            "static_nodes": int(nodes_allocated[0]),
            "verification_nodes": int(nodes_allocated[-1])
        }
    return None

def run_benchmarks():
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    results_path = BENCHMARK_DIR / f"results_{timestamp}"
    results_path.mkdir(parents=True, exist_ok=True)

    csv_final_path = results_path / "final_benchmarks.csv"
    headers = [
        "model", "algorithm", "total_ms", "compile_ms", "verify_ms",
        "static_nodes", "verification_nodes", "peak_mem_kb", "states"
    ]

    all_data = []

    for model_name in INSTANCES:
        smv_file = EXAMPLES_DIR / model_name
        if not smv_file.exists():
            print(f"File not found: {model_name}")
            continue

        print(f"\n>>> Processing Model: {model_name}")

        for algo in CHECKER_ALGORITHMS:
            try:
                print(f"    Running SSMV-Checker with algorithm: {algo}...")
                temp_csv = PROJECT_ROOT / "benchmarks.csv"
                if temp_csv.exists(): os.remove(temp_csv)

                cmd_checker = ["time", "-v", str(CHECKER_BIN), "verify", str(smv_file), "--algorithm", algo]

                proc_c = subprocess.run(cmd_checker, capture_output=True, text=True, timeout=TIMEOUT_LIMIT)

                peak_c = get_peak_mem(proc_c.stderr)

                if proc_c.returncode == 0 and temp_csv.exists():
                    with open(temp_csv, 'r') as f:
                        row = list(csv.DictReader(f))[-1]
                        all_data.append([
                            model_name, f"SSMV-{algo.capitalize()}", row['total_ms'], row['compile_ms'],
                            row['verify_ms'], row['static_nodes'], row['verification_nodes'], peak_c, row.get('states', 0)
                        ])
                else:
                    all_data.append([model_name, f"SSMV-{algo.capitalize()}", "ERROR/OOM", "-", "-", "-", "-", peak_c, "-"])

            except subprocess.TimeoutExpired:
                print(f"    TIMEOUT: {algo} excedeu {TIMEOUT_LIMIT}s")
                all_data.append([model_name, f"SSMV-{algo.capitalize()}", "TIMEOUT", "-", "-", "-", "-", "-", "-"])

        try:
            print(f"    Running NuSMV benchmark...")
            nusmv_script = results_path / f"bench_{model_name}.nusmv"
            with open(nusmv_script, 'w') as f:
                f.write(NUSMV_TEMPLATE.format(model_path=str(smv_file)))

            cmd_nusmv = ["time", "-v", "NuSMV", "-dcx", "-mono", "-coi", "-source", str(nusmv_script)]

            proc_n = subprocess.run(cmd_nusmv, capture_output=True, text=True, timeout=TIMEOUT_LIMIT)

            peak_n = get_peak_mem(proc_n.stderr)
            stats_n = parse_nusmv_output(proc_n.stdout)

            if stats_n:
                all_data.append([
                    model_name, "NuSMV", stats_n['total_ms'], stats_n['compile_ms'],
                    stats_n['verify_ms'], stats_n['static_nodes'], stats_n['verification_nodes'], peak_n, 0
                ])
            else:
                all_data.append([model_name, "NuSMV", "ERROR/OOM", "-", "-", "-", "-", peak_n, "-"])

        except subprocess.TimeoutExpired:
            print(f"    TIMEOUT: NuSMV excedeu {TIMEOUT_LIMIT}s")
            all_data.append([model_name, "NuSMV", "TIMEOUT", "-", "-", "-", "-", "-", "-"])

    with open(csv_final_path, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(headers)
        writer.writerows(all_data)

    print(f"\nDone! All results consolidated in: {results_path}")

if __name__ == "__main__":
    run_benchmarks()
