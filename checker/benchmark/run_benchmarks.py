
#!/usr/bin/env python3
"""
run_experiments_suite.py
================================================================================
Advanced Benchmarking Framework for Hardware Model Checking Paradigms.

Features:
  - Adaptive Saturation Shortcutting: Prunes future N values if an engine fails.
  - Dynamic seed scaling inside the sample loop for random heuristics.
  - Native CLI flag injection (-dynamic) for NuSMV to prevent shell deadlocks.
  - Asymmetric non-linear cross-killing engine for BDD solvers.
  - Process Group Isolation: Eradicates ghost zombie background processes.
"""

import os
import sys
import re
import csv
import math
import time
import signal
import argparse
import subprocess
import resource
from datetime import datetime
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple, Set

# ─── ENVIRONMENT DISCOVERIES ──────────────────────────────────────────────────
PROJECT_ROOT = Path(__file__).parent.absolute() if Path(__file__).parent.name != "benchmark" else Path(__file__).parent.parent.absolute()
EXAMPLES_DIR = PROJECT_ROOT / "data" / "raw"
BENCHMARK_DIR = PROJECT_ROOT / "benchmark"
CHECKER_BIN = "checker"
TIMEOUT_LIMIT = 60

NUSMV_TEMPLATE = """
set default_trace_plugin 0
set cone_of_influence
set bdd_static_order_heuristics none
read_model -i {model_path}
flatten_hierarchy
encode_variables
build_model -m Monolithic
echo "[MILESTONE 1]"
print_usage
check_ctlspec
echo "[MILESTONE 2]"
print_usage
{write_order_line}
quit
"""

def NUSMV_TEMPLATE_EXEC(model_path: str, write_order_line: str) -> str:
    return NUSMV_TEMPLATE.format(
        model_path=model_path,
        write_order_line=write_order_line
    )

# ─── HELPER EXTRACTION UTILITIES ──────────────────────────────────────────────
def get_peak_mem_kb(stderr_output: str) -> int:
    match = re.search(r"Maximum resident set size \(kbytes\): (\d+)", stderr_output)
    return int(match.group(1)) if match else 0

def parse_nusmv_output(stdout_output: str) -> Optional[Dict[str, int]]:
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

def extract_n_from_filename(filename: str) -> int:
    match = re.search(r"_(\d+)\.smv$", filename)
    return int(match.group(1)) if match else 0

def compute_dynamic_timeout(leader_time: float) -> float:
    if leader_time < 0.05:
        return 5.0
    return leader_time + 10.0 * math.sqrt(leader_time) + 5.0

def safe_extract_metric(metrics_dict: Dict[str, Any], keys: List[str]) -> Any:
    for key in keys:
        if key in metrics_dict:
            return metrics_dict[key]
    return "-"
def limit_memory():
    """Define o limite máximo de memória (Address Space) para 5 GB por processo."""
    # 10 GB em bytes = 5 * 1024 * 1024 * 1024
    five_gb = 10 * 1024 * 1024 * 1024
    # Configura tanto o limite Soft quanto o Hard
    resource.setrlimit(resource.RLIMIT_AS, (five_gb, five_gb))

# ─── PARALELISMO ASSÍNCRONO COM CROSS-KILLING ──────────────────────────────────
def monitor_bdd_asymmetric_parallel(
    cmd_checker: List[str],
    cmd_nusmv: List[str],
    temp_csv_path: Path,
    checker_already_sated: bool,
    nusmv_already_sated: bool,
    nusmv_tag: str
) -> Tuple[Dict[str, Any], Dict[str, Any]]:
    """Roda os motores BDD em concorrência controlada, respeitando podas prévias."""
    checker_res = {"status": "TIMEOUT", "total_ms": "-", "compile_ms": "-", "verify_ms": "-", "static_nodes": "-", "verification_nodes": "-", "peak_mem": 0, "states": 0}
    nusmv_res = {"status": "TIMEOUT", "total_ms": "-", "compile_ms": "-", "verify_ms": "-", "static_nodes": "-", "verification_nodes": "-", "peak_mem": 0, "states": 0}

    if checker_already_sated and nusmv_already_sated:
        checker_res["status"] = "TIMEOUT"
        nusmv_res["status"] = "TIMEOUT"
        return checker_res, nusmv_res

    start_global = time.time()
    p_checker = None
    p_nusmv = None

    # Inicialização condicional por Process Group e Limite de RAM
    if not checker_already_sated:
        p_checker = subprocess.Popen(
            ["time", "-v"] + cmd_checker,
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True, cwd=str(PROJECT_ROOT),
            preexec_fn=lambda: [os.setpgrp(), limit_memory()] # <-- LIMITA AQUI
        )
    else:
        checker_res["status"] = "TIMEOUT"

    if not nusmv_already_sated:
        p_nusmv = subprocess.Popen(
            ["time", "-v"] + cmd_nusmv,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=str(PROJECT_ROOT),
            preexec_fn=lambda: [os.setpgrp(), limit_memory()] # <-- LIMITA AQUI
        )
    else:
        nusmv_res["status"] = "TIMEOUT"

    leader_time: Optional[float] = None
    leader_engine: Optional[str] = None

    while True:
        elapsed = time.time() - start_global

        if elapsed > TIMEOUT_LIMIT:
            if p_checker and p_checker.poll() is None:
                try: os.killpg(os.getpgid(p_checker.pid), signal.SIGKILL)
                except: pass
            if p_nusmv and p_nusmv.poll() is None:
                try: os.killpg(os.getpgid(p_nusmv.pid), signal.SIGKILL)
                except: pass
            break

        # Monitoriza o Checker
        if p_checker and p_checker.poll() is not None and checker_res["status"] == "TIMEOUT":
            duration = time.time() - start_global
            _, stderr = p_checker.communicate()
            checker_res["peak_mem"] = get_peak_mem_kb(stderr)

            if p_checker.returncode == 0 and temp_csv_path.exists():
                with open(temp_csv_path, 'r') as f:
                    metrics = list(csv.DictReader(f))[-1]

                checker_res.update({
                    "status": "SUCCESS",
                    "total_ms": safe_extract_metric(metrics, ['total_time_ms', 'total_ms']),
                    "compile_ms": safe_extract_metric(metrics, ['compilation_time_ms', 'compile_ms']),
                    "verify_ms": safe_extract_metric(metrics, ['verification_time_ms', 'verify_ms']),
                    "static_nodes": safe_extract_metric(metrics, ['static_nodes', 'static_node_count']),
                    "verification_nodes": safe_extract_metric(metrics, ['verification_nodes', 'verification_node_count']),
                    "states": safe_extract_metric(metrics, ['explicit_states', 'states', 'states_explored'])
                })
                if leader_engine is None:
                    leader_engine = "checker"
                    leader_time = duration
            else:
                checker_res["status"] = "CRASH"

        # Monitoriza o NuSMV
        if p_nusmv and p_nusmv.poll() is not None and nusmv_res["status"] == "TIMEOUT":
            duration = time.time() - start_global
            stdout, stderr = p_nusmv.communicate()
            nusmv_res["peak_mem"] = get_peak_mem_kb(stderr)
            stats = parse_nusmv_output(stdout)

            if stats:
                nusmv_res.update({"status": "SUCCESS", "total_ms": stats['total_ms'], "compile_ms": stats['compile_ms'], "verify_ms": stats['verify_ms'], "static_nodes": stats['static_nodes'], "verification_nodes": stats['verification_nodes']})
                if leader_engine is None:
                    leader_engine = "nusmv"
                    leader_time = duration
            else:
                nusmv_res["status"] = "CRASH"

        # Cross-killing cirúrgico via os.killpg
        if leader_engine is not None and leader_time is not None:
            max_window = compute_dynamic_timeout(leader_time)
            if elapsed > max_window:
                if p_checker and p_checker.poll() is None:
                    try: os.killpg(os.getpgid(p_checker.pid), signal.SIGKILL)
                    except: pass
                    checker_res["status"] = "STOPPED"
                if p_nusmv and p_nusmv.poll() is None:
                    try: os.killpg(os.getpgid(p_nusmv.pid), signal.SIGKILL)
                    except: pass
                    nusmv_res["status"] = "STOPPED"
                break

        checker_done = (p_checker is None) or (p_checker.poll() is not None)
        nusmv_done = (p_nusmv is None) or (p_nusmv.poll() is not None)
        if checker_done and nusmv_done:
            break

        time.sleep(0.05)

    return checker_res, nusmv_res

# ─── CORE PIPELINE ORCHESTRATOR ───────────────────────────────────────────────
def execute_benchmark_suite(problem_name: str, heuristics: List[str], iterations: int, algorithms: List[str]):
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    problem_results_dir = BENCHMARK_DIR / f"{problem_name}_results_{timestamp}"
    problem_results_dir.mkdir(parents=True, exist_ok=True)

    csv_output_path = problem_results_dir / f"benchmarks_{problem_name}.csv"
    headers = [
        "Problem", "N", "Heuristic", "Seed_Or_Iter", "Iteration", "Algorithm",
        "Total_MS", "Compile_MS", "Verify_MS", "Static_Nodes", "Verification_Nodes",
        "Peak_Mem_KB", "States_Explored", "Status"
    ]

    all_files = os.listdir(EXAMPLES_DIR) if EXAMPLES_DIR.exists() else []
    target_instances = [f for f in all_files if f.startswith(f"{problem_name}_") and f.endswith(".smv")]
    target_instances.sort(key=extract_n_from_filename)

    if not target_instances:
        print(f"❌ No matching generated models found for: '{problem_name}_N.smv' inside {EXAMPLES_DIR}")
        return

    print(f"🚀 Initializing Benchmark Engine for Problem Group: [{problem_name.upper()}]")
    print(f"📂 Storing reports under: {problem_results_dir}")

    with open(csv_output_path, mode='w', newline='') as csv_file:
        writer = csv.writer(csv_file)
        writer.writerow(headers)

    sated_algorithms: Dict[str, Set[str]] = {h: set() for h in heuristics}

    for instance_file in target_instances:
        n_val = extract_n_from_filename(instance_file)
        model_full_path = EXAMPLES_DIR / instance_file
        print(f"\n============ Analyzing Instance: {instance_file} (N = {n_val}) ============")

        for heuristic in heuristics:
            print(f"  💡 Variable Ordering Strategy: [{heuristic.upper()}]")
            is_random_heuristic = heuristic.startswith("random") or heuristic == "random"

            base_seed = 42
            param_val = "-"
            if heuristic.startswith("random:"):
                base_seed = int(heuristic.split(":")[1])
                param_val = str(base_seed)
            elif heuristic.startswith("force:"):
                param_val = heuristic.split(":")[1]

            instance_stem = instance_file.replace(".smv", "")
            shared_ord_file = PROJECT_ROOT / f"ord_{instance_stem}.ord"

            if shared_ord_file.exists(): os.remove(shared_ord_file)
            has_valid_order_file = False

            # ─── ESTÁGIO DE PRÉ-GERAÇÃO DA ORDEM ───
            if not is_random_heuristic:
                nusmv_tag = "NuSMV-DynamicSifting" if heuristic == "dynamic" else "NuSMV-Standard"
                gen_sated = ("SSMV-BDD" in sated_algorithms[heuristic]) if (heuristic == "default" or heuristic.startswith("force")) else (nusmv_tag in sated_algorithms[heuristic])

                if not gen_sated:
                    if heuristic == "default" or heuristic.startswith("force"):
                        cmd_gen = [CHECKER_BIN, "verify", str(model_full_path), "--order", heuristic, "--export-order", "export-only"]
                        try:
                            subprocess.run(cmd_gen, capture_output=True, text=True, timeout=30, cwd=str(PROJECT_ROOT))
                            if shared_ord_file.exists():
                                has_valid_order_file = True
                        except Exception as e:
                            print(f"    ⚠️ Failed to pre-generate ordering vector: {e}")

                    elif heuristic == "dynamic":
                        script_sift_path = problem_results_dir / f"tmp_sift_{instance_file}.nusmv"
                        with open(script_sift_path, 'w') as sf:
                            sf.write(NUSMV_TEMPLATE_EXEC(model_path=str(model_full_path), write_order_line=f"write_order -o {shared_ord_file}"))

                        cmd_sift = ["NuSMV", "-dynamic", "-dcx", "-mono", "-coi", "-source", str(script_sift_path)]
                        try:
                            subprocess.run(cmd_sift, capture_output=True, text=True, timeout=TIMEOUT_LIMIT, cwd=str(PROJECT_ROOT))
                            if shared_ord_file.exists():
                                with open(shared_ord_file, "r") as f_ord:
                                    clean_lines = [line.split(":")[-1].strip() for line in f_ord if line.strip()]
                                with open(shared_ord_file, "w") as f_ord:
                                    f_ord.write("\n".join(clean_lines))
                                has_valid_order_file = True
                                print(f"    ✅ Intercepted & Cleaned NuSMV Sifting matrix: {shared_ord_file.name}")
                        except Exception as e:
                            print(f"    ⚠️ NuSMV order interception stalled: {e}")
                        finally:
                            if script_sift_path.exists(): os.remove(script_sift_path)

            # ─── BATERIA DE ITERAÇÕES (MÚLTIPLAS AMOSTRAS) ───
            for current_iter in range(1, iterations + 1):
                print(f"    🔄 Sample Loop [{current_iter}/{iterations}]")
                rows_to_append = []

                if is_random_heuristic and ("SSMV-BDD" not in sated_algorithms[heuristic]):
                    current_seed = base_seed + current_iter
                    param_val = str(current_seed)
                    heuristic_arg = f"random:{current_seed}"

                    if shared_ord_file.exists(): os.remove(shared_ord_file)
                    has_valid_order_file = False

                    cmd_gen = [CHECKER_BIN, "verify", str(model_full_path), "--order", heuristic_arg, "--export-order", "export-only"]
                    try:
                        res = subprocess.run(cmd_gen, capture_output=True, text=True, timeout=30, cwd=str(PROJECT_ROOT))
                        if shared_ord_file.exists():
                            has_valid_order_file = True
                        else:
                            if res.returncode == 0 and len(res.stdout.strip()) > 0:
                                lines = [line.strip() for line in res.stdout.split('\n') if line.strip() and not "Using" in line and not "Variable" in line]
                                if lines:
                                    with open(shared_ord_file, "w") as f_ord:
                                        f_ord.write("\n".join(lines))
                                    has_valid_order_file = True
                    except Exception as e:
                        print(f"        ⚠️ Failed to generate random order for loop sample {current_iter}: {e}")

                # --- PARADIGMA 1: MOTORES DE BDD ---
                if "bdd" in algorithms:
                    nusmv_tag = "NuSMV-DynamicSifting" if heuristic == "dynamic" else "NuSMV-Standard"
                    c_sated = "SSMV-BDD" in sated_algorithms[heuristic]
                    n_sated = nusmv_tag in sated_algorithms[heuristic]

                    temp_csv = PROJECT_ROOT / "benchmarks.csv"
                    if temp_csv.exists(): os.remove(temp_csv)

                    cmd_checker = [CHECKER_BIN, "verify", str(model_full_path), "--algorithm", "bdd"]
                    cmd_checker += ["--order", str(shared_ord_file)] if has_valid_order_file else ["--order", "default"]

                    script_runner_path = problem_results_dir / f"run_BDD_parallel_{instance_file}_iter_{current_iter}.nusmv"
                    with open(script_runner_path, 'w') as rf:
                        rf.write(NUSMV_TEMPLATE_EXEC(model_path=str(model_full_path), write_order_line=""))

                    cmd_nusmv = ["NuSMV", "-dcx", "-mono", "-coi"]
                    if heuristic == "dynamic": cmd_nusmv.append("-dynamic")
                    elif has_valid_order_file and shared_ord_file.exists(): cmd_nusmv += ["-i", str(shared_ord_file)]
                    cmd_nusmv += ["-source", str(script_runner_path)]

                    c_res, n_res = monitor_bdd_asymmetric_parallel(cmd_checker, cmd_nusmv, temp_csv, c_sated, n_sated, nusmv_tag)

                    if c_res["status"] in ["TIMEOUT", "STOPPED", "CRASH"] and not c_sated:
                        print(f"    [SATURATION ALERT] Engine [SSMV-BDD] sated at N={n_val}. Pruning future entries.")
                        sated_algorithms[heuristic].add("SSMV-BDD")
                    if n_res["status"] in ["TIMEOUT", "STOPPED", "CRASH"] and not n_sated:
                        print(f"    [SATURATION ALERT] Engine [{nusmv_tag}] sated at N={n_val}. Pruning future entries.")
                        sated_algorithms[heuristic].add(nusmv_tag)

                    rows_to_append.append([
                        problem_name, n_val, heuristic, param_val, current_iter, "SSMV-BDD",
                        c_res["total_ms"], c_res["compile_ms"], c_res["verify_ms"], c_res["static_nodes"], c_res["verification_nodes"], c_res["peak_mem"], c_res["states"], c_res["status"]
                    ])
                    rows_to_append.append([
                        problem_name, n_val, heuristic, param_val, current_iter, nusmv_tag,
                        n_res["total_ms"], n_res["compile_ms"], n_res["verify_ms"], n_res["static_nodes"], n_res["verification_nodes"], n_res["peak_mem"], 0, n_res["status"]
                    ])

                    if script_runner_path.exists(): os.remove(script_runner_path)

                    # ITEM ADICIONAL 2: Variante NuSMV-StaticImport
                    if heuristic == "dynamic":
                        si_sated = "NuSMV-StaticImport" in sated_algorithms[heuristic]
                        if not si_sated and n_res["status"] == "SUCCESS" and shared_ord_file.exists():
                            script_static_path = problem_results_dir / f"run_NuSMV_StaticImport_{instance_file}_iter_{current_iter}.nusmv"
                            with open(script_static_path, 'w') as rf:
                                rf.write(NUSMV_TEMPLATE_EXEC(model_path=str(model_full_path), write_order_line=""))

                            cmd_nusmv_static = ["time", "-v", "NuSMV", "-dcx", "-mono", "-coi", "-i", str(shared_ord_file), "-source", str(script_static_path)]
                            try:
                                proc_st = subprocess.run(cmd_nusmv_static, capture_output=True, text=True, timeout=TIMEOUT_LIMIT, cwd=str(PROJECT_ROOT))
                                st_peak = get_peak_mem_kb(proc_st.stderr)
                                st_stats = parse_nusmv_output(proc_st.stdout)
                                if st_stats:
                                    rows_to_append.append([
                                        problem_name, n_val, heuristic, param_val, current_iter, "NuSMV-StaticImport",
                                        st_stats['total_ms'], st_stats['compile_ms'], st_stats['verify_ms'], st_stats['static_nodes'], st_stats['verification_nodes'], st_peak, 0, "SUCCESS"
                                    ])
                                else:
                                    rows_to_append.append([problem_name, n_val, heuristic, param_val, current_iter, "NuSMV-StaticImport", "ERROR", "-", "-", "-", "-", st_peak, 0, "CRASH"])
                                    sated_algorithms[heuristic].add("NuSMV-StaticImport")
                            except subprocess.TimeoutExpired:
                                rows_to_append.append([problem_name, n_val, heuristic, param_val, current_iter, "NuSMV-StaticImport", "TIMEOUT", "-", "-", "-", "-", "-", 0, "TIMEOUT"])
                                sated_algorithms[heuristic].add("NuSMV-StaticImport")
                            finally:
                                if script_static_path.exists(): os.remove(script_static_path)
                        else:
                            rows_to_append.append([problem_name, n_val, heuristic, param_val, current_iter, "NuSMV-StaticImport", "TIMEOUT", "-", "-", "-", "-", "-", 0, "TIMEOUT"])

                # --- PARADIGMA 2: LABELLING-SCC ---
                if "labelling-scc" in algorithms:
                    exp_sated = "SSMV-LABELLING-SCC" in sated_algorithms[heuristic]

                    if not exp_sated:
                        cmd_explicit = ["time", "-v", CHECKER_BIN, "verify", str(model_full_path), "--algorithm", "labelling-scc"]
                        temp_csv = PROJECT_ROOT / "benchmarks.csv"
                        if temp_csv.exists(): os.remove(temp_csv)

                        try:
                            proc_exp = subprocess.run(cmd_explicit, capture_output=True, text=True, timeout=TIMEOUT_LIMIT, cwd=str(PROJECT_ROOT))
                            peak_mem = get_peak_mem_kb(proc_exp.stderr)

                            if proc_exp.returncode == 0 and temp_csv.exists():
                                with open(temp_csv, 'r') as f:
                                    metrics = list(csv.DictReader(f))[-1]

                                total = safe_extract_metric(metrics, ['total_time_ms', 'total_ms'])
                                comp = safe_extract_metric(metrics, ['compilation_time_ms', 'compile_ms'])
                                verif = safe_extract_metric(metrics, ['verification_time_ms', 'verify_ms'])
                                s_nodes = safe_extract_metric(metrics, ['static_nodes', 'static_node_count'])
                                v_nodes = safe_extract_metric(metrics, ['verification_nodes', 'verification_node_count'])
                                states = safe_extract_metric(metrics, ['explicit_states', 'states', 'states_explored'])

                                rows_to_append.append([
                                    problem_name, n_val, heuristic, param_val, current_iter, "SSMV-LABELLING-SCC",
                                    total, comp, verif, s_nodes, v_nodes, peak_mem, states, "SUCCESS"
                                ])
                            else:
                                rows_to_append.append([problem_name, n_val, heuristic, param_val, current_iter, "SSMV-LABELLING-SCC", "ERROR", "-", "-", "-", "-", peak_mem, "-", "CRASH"])
                                print(f"    [SATURATION ALERT] Engine [SSMV-LABELLING-SCC] crashed at N={n_val}. Pruning future entries.")
                                sated_algorithms[heuristic].add("SSMV-LABELLING-SCC")
                        except subprocess.TimeoutExpired:
                            rows_to_append.append([problem_name, n_val, heuristic, param_val, current_iter, "SSMV-LABELLING-SCC", "TIMEOUT", "-", "-", "-", "-", "-", "-", "TIMEOUT"])
                            print(f"    [SATURATION ALERT] Engine [SSMV-LABELLING-SCC] timed out at N={n_val}. Pruning future entries.")
                            sated_algorithms[heuristic].add("SSMV-LABELLING-SCC")
                        finally:
                            if temp_csv.exists(): os.remove(temp_csv)
                    else:
                        rows_to_append.append([problem_name, n_val, heuristic, param_val, current_iter, "SSMV-LABELLING-SCC", "TIMEOUT", "-", "-", "-", "-", "-", "-", "TIMEOUT"])

                with open(csv_output_path, mode='a', newline='') as csv_file:
                    writer = csv.writer(csv_file)
                    writer.writerows(rows_to_append)

            if shared_ord_file.exists(): os.remove(shared_ord_file)

    print(f"\n✅ Experiment suite successfully completed for problem catalog: [{problem_name.upper()}].")

# ─── COMMAND LINE INTERFACE ENTRY POINT ───────────────────────────────────────
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Advanced Benchmarking Framework for Hardware Model Checking Paradigms.")
    parser.add_argument("--problem", type=str, required=True, help="The target base name of the problem (e.g., 'dining', 'counter').")
    parser.add_argument("--heuristics", type=str, default="default,random:42,force:500,dynamic", help="Comma-separated variable ordering strategies.")
    parser.add_argument("--iterations", type=int, default=30, help="Number of multi-sample iterations per configuration grid.")
    parser.add_argument("--algorithms", type=str, default="bdd,labelling-scc", help="Comma-separated checker verification engine paradigms.")

    args = parser.parse_args()

    heuristics_list = [h.strip() for h in args.heuristics.split(",")]
    algorithms_list = [a.strip() for a in args.algorithms.split(",")]

    execute_benchmark_suite(
        problem_name=args.problem,
        heuristics=heuristics_list,
        iterations=args.iterations,
        algorithms=algorithms_list
    )
