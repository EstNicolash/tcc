"""
find_max_n.py
======================
Automated Parameter Scaling and Saturation Discovery Framework for TCC Benchmarks.

This script executes an Exponential Step Search followed by a Binary Refinement
across model types to discover the maximum sustainable parameter size (N_max).

High-Performance Parallel Engine:
  - Multi-instance concurrent execution pool scales processing across CPU cores.
  - Asymmetric paradigm isolation (Explicit isolated vs BDD parallel with cross-killing).
  - Thread-safe incremental streaming directly into the target CSV report.
"""

import subprocess
import time
import math
import csv
import os
from concurrent.futures import ThreadPoolExecutor, as_completed
import threading  # Injetado para prover exclusão mútua na escrita do CSV
from typing import Dict, Union, List, Any, Optional

# ─── Configuration Space ──────────────────────────────────────────────────────
TIMEOUT_PER_INSTANCE = 60.0  # Tempo limite estrito para o motor explícito isolado
ABSOLUTE_HARD_TIMEOUT = 300.0  # 5 minutos de limite duro global de segurança para BDDs
MAX_PARALLEL_MODELS = 8
CSV_OUTPUT_PATH = "saturation_report.csv"
RAW_MODELS_DIR = "raw"  # Target location for generated persistent files

os.makedirs(RAW_MODELS_DIR, exist_ok=True)
csv_writer_lock = threading.Lock()  # Lock para garantir concorrência segura no CSV

def compute_dynamic_timeout(leader_time: float) -> float:
    """Amortized non-linear timeout: leader + 10 * sqrt(leader) + 5s margin"""
    if leader_time is None or leader_time < 0.05:
        return 10.0
    return leader_time + 10.0 * math.sqrt(leader_time) + 5.0

# ─── Individual Process Driver ───────────────────────────────────────────────
def run_command(cmd_args: List[str]):
    start = time.time()
    proc = subprocess.Popen(
        cmd_args,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL
    )
    return proc, start

# ─── Pure Isolated Explicit Runner ────────────────────────────────────────────
def evaluate_explicit_isolated(smv_file: str) -> Union[float, str]:
    """Runs the explicit engine in absolute isolation with a strict static timeout."""
    cmd = ["checker", "verify", smv_file, "--algorithm", "labelling-scc"]
    start_time = time.time()
    proc: Optional[subprocess.Popen] = None
    try:
        proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        proc.wait(timeout=TIMEOUT_PER_INSTANCE)
        elapsed = time.time() - start_time
        return elapsed if proc.returncode == 0 else "FAILED"
    except subprocess.TimeoutExpired:
        if proc is not None:
            proc.kill()
            proc.wait()
        return "TIMEOUT"
    except Exception:
        if proc is not None:
            proc.kill()
            proc.wait()
        return "FAILED"

# ─── Parallel BDD Orchestrator (NuSMV vs Your Symbolic Engine) ────────────────
def evaluate_bdd_concurrent(smv_file: str, active_engines: List[str]) -> Dict[str, Union[float, str, None]]:
    """Executes NuSMV and Your Symbolic Engine in parallel with cross-killing."""
    results: Dict[str, Union[float, str, None]] = {"nusmv": None, "symbolic": None}
    handles: Dict[str, subprocess.Popen] = {}
    timestamps = {}

    commands = {
        "nusmv": ["NuSMV", "-coi", "-mono", "-dcx", "-dynamic", smv_file],
        "symbolic": ["checker", "verify", smv_file, "--order", "force:500"]
    }

    for engine in active_engines:
        if engine in commands:
            handles[engine], timestamps[engine] = run_command(commands[engine])

    global_start = time.time()
    leader_engine: Optional[str] = None
    leader_time: Optional[float] = None

    while True:
        elapsed = time.time() - global_start

        if elapsed > ABSOLUTE_HARD_TIMEOUT:
            for eng, h in handles.items():
                if h.poll() is None: h.kill()
            break

        for engine in list(handles.keys()):
            handle = handles[engine]
            if handle.poll() is not None:
                if results[engine] is None:
                    duration = time.time() - timestamps[engine]
                    if handle.returncode == 0:
                        results[engine] = duration
                        if leader_engine is None:
                            leader_engine = engine
                            leader_time = duration
                    else:
                        results[engine] = "FAILED"

                    del handles[engine]

                if leader_engine is not None and leader_time is not None:
                    max_allowed_window = compute_dynamic_timeout(leader_time)
                    if elapsed > max_allowed_window:
                        for eng, h in handles.items():
                            if h.poll() is None: h.kill()
                        return results

        if not handles:
            break

        time.time()
        time.sleep(0.05)

    return results

# ─── Verification Wrapper ─────────────────────────────────────────────────────
def get_smv_model_path(n: int, model_name: str, generator) -> Optional[str]:
    """Generates the model if not present and returns the persistent path."""
    target_smv_path = os.path.join(RAW_MODELS_DIR, f"{model_name}_{n}.smv")
    if not os.path.exists(target_smv_path):
        try:
            content = generator(n)
            with open(target_smv_path, "w") as f:
                f.write(content)
        except Exception as e:
            print(f"    Error generating model {model_name} at N={n}: {e}")
            return None
    return target_smv_path

# ─── Core Unit Work Task Function ─────────────────────────────────────────────
def discover_saturation_for_model(model_name: str, generator) -> str:
    """Executes the full pipeline for a specific protocol instance."""
    print(f"[START] Processing saturation bounds for: {model_name}")

    if model_name in ["rule30", "mcs", "train_gate", "synapse", "fischer", "elevator2", "brp2"]:
        base_n = 3
    elif model_name in ["bad_order"]:
        base_n = 5
    else:
        base_n = 2

    master_metrics: Dict[int, Dict[str, Any]] = {}

    # ESTÁGIO 1: Motor Explícito Isolado
    lower_bound = base_n
    upper_bound = base_n
    is_first_step = True
    max_explicit_n = base_n

    while True:
        n = lower_bound if is_first_step else upper_bound
        smv_file = get_smv_model_path(n, model_name, generator)
        if not smv_file: break

        result = evaluate_explicit_isolated(smv_file)
        if n not in master_metrics:
            master_metrics[n] = {"explicit": "STOPPED", "symbolic": "STOPPED", "nusmv": "STOPPED"}
        master_metrics[n]["explicit"] = result

        if result == "TIMEOUT" or result == "FAILED":
            if is_first_step: upper_bound = n
            break
        else:
            max_explicit_n = max(max_explicit_n, n)
            lower_bound = n
            if is_first_step:
                upper_bound = base_n * 2
                is_first_step = False
            else:
                upper_bound *= 2
        if upper_bound > 128:
            upper_bound = 128
            break

    if lower_bound < upper_bound and upper_bound > base_n:
        low = lower_bound + 1
        high = upper_bound - 1
        while low <= high:
            mid = (low + high) // 2
            smv_file = get_smv_model_path(mid, model_name, generator)
            if not smv_file: break
            result = evaluate_explicit_isolated(smv_file)
            if mid not in master_metrics:
                master_metrics[mid] = {"explicit": "STOPPED", "symbolic": "STOPPED", "nusmv": "STOPPED"}
            master_metrics[mid]["explicit"] = result
            if result == "TIMEOUT" or result == "FAILED":
                high = mid - 1
            else:
                max_explicit_n = max(max_explicit_n, mid)
                low = mid + 1

    # ESTÁGIO 2: Motores BDD Concorrentes
    lower_bound = base_n
    upper_bound = base_n
    is_first_step = True
    max_bdd_n = base_n

    while True:
        n = lower_bound if is_first_step else upper_bound
        smv_file = get_smv_model_path(n, model_name, generator)
        if not smv_file: break

        bdd_metrics = evaluate_bdd_concurrent(smv_file, ["nusmv", "symbolic"])
        if n not in master_metrics:
            master_metrics[n] = {"explicit": "STOPPED", "symbolic": "STOPPED", "nusmv": "STOPPED"}

        master_metrics[n]["nusmv"] = bdd_metrics["nusmv"]
        master_metrics[n]["symbolic"] = bdd_metrics["symbolic"]

        if (bdd_metrics["nusmv"] in [None, "FAILED"]) and (bdd_metrics["symbolic"] in [None, "FAILED"]):
            if is_first_step: upper_bound = n
            break
        else:
            max_bdd_n = max(max_bdd_n, n)
            lower_bound = n
            if is_first_step:
                upper_bound = base_n * 2
                is_first_step = False
            else:
                upper_bound *= 2
        if upper_bound > 128:
            upper_bound = 128
            break

    if lower_bound < upper_bound and upper_bound > base_n:
        low = lower_bound + 1
        high = upper_bound - 1
        while low <= high:
            mid = (low + high) // 2
            smv_file = get_smv_model_path(mid, model_name, generator)
            if not smv_file: break
            bdd_metrics = evaluate_bdd_concurrent(smv_file, ["nusmv", "symbolic"])
            if mid not in master_metrics:
                master_metrics[mid] = {"explicit": "STOPPED", "symbolic": "STOPPED", "nusmv": "STOPPED"}
            master_metrics[mid]["nusmv"] = bdd_metrics["nusmv"]
            master_metrics[mid]["symbolic"] = bdd_metrics["symbolic"]
            if (bdd_metrics["nusmv"] in [None, "FAILED"]) and (bdd_metrics["symbolic"] in [None, "FAILED"]):
                high = mid - 1
            else:
                max_bdd_n = max(max_bdd_n, mid)
                low = mid + 1

    # Despeja de forma segura usando exclusão mútua (Lock) no arquivo CSV
    with csv_writer_lock:
        with open(CSV_OUTPUT_PATH, mode='a', newline='') as csv_file:
            writer = csv.writer(csv_file)
            for n_val in sorted(master_metrics.keys()):
                m_exp = master_metrics[n_val]["explicit"]
                m_sym = master_metrics[n_val]["symbolic"]
                m_nusmv = master_metrics[n_val]["nusmv"]

                def fmt(v):
                    if v is None: return "TIMEOUT"
                    return f"{v:.2f}s" if isinstance(v, float) else str(v)

                print(f"    [{model_name}] N = {n_val} -> Expl: {fmt(m_exp)} | Simb: {fmt(m_sym)} | NuSMV: {fmt(m_nusmv)}")
                writer.writerow([model_name, n_val, m_nusmv, m_sym, m_exp])

    return model_name

# ─── Main Orchestrator ────────────────────────────────────────────────────────
def main():
    print("=== Launching High-Performance Multi-Instance Parallel Framework ===")
    print(f"Artifacts persistent storage directory initialized at: ./{RAW_MODELS_DIR}/")
    print(f"Concurrent instance workers pool size: {MAX_PARALLEL_MODELS}\n")

    import bad_order, bakery, brp2, coi_chaos_killer, coi_killer, counter, dining, elevator2, firewire_link, fischer, mcs, rule30, sokoban, synapse, train_gate

    benchmarks = {
        "bad_order": bad_order.generate_instance,
        "bakery": bakery.generate_instance,
        "brp2": brp2.generate_instance,
        "coi_chaos_killer": coi_chaos_killer.generate_instance,
        "coi_killer": coi_killer.generate_instance,
        "counter": counter.generate_instance,
        "dining": dining.generate_instance,
        "elevator2": elevator2.generate_instance,
        "firewire_link": firewire_link.generate_instance,
        "fischer": fischer.generate_instance,
        "mcs": mcs.generate_instance,
        "rule30": rule30.generate_instance,
        "sokoban": sokoban.generate_instance,
        "synapse": synapse.generate_instance,
        "train_gate": train_gate.generate_instance
    }

    # Inicializa as colunas do CSV de forma limpa no início
    with open(CSV_OUTPUT_PATH, mode='w', newline='') as csv_file:
        writer = csv.writer(csv_file)
        writer.writerow(["Model", "N", "NuSMV_Time", "Symbolic_Time", "Explicit_Time"])

    # Dispara o pool de execução concorrente multi-instância por modelo
    with ThreadPoolExecutor(max_workers=MAX_PARALLEL_MODELS) as executor:
        futures = [executor.submit(discover_saturation_for_model, name, gen) for name, gen in benchmarks.items()]

        for future in as_completed(futures):
            completed_model = future.result()
            print(f"[COMPLETED] Evaluation matrix fully generated for: {completed_model}")

    print(f"\n=== Exploration Complete. Output stored in '{CSV_OUTPUT_PATH}' ===")

if __name__ == "__main__":
    main()
