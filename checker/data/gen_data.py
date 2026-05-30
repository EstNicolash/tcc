"""
generate_benchmarks_batch.py
================================================================================
Orchestrator for Parameterized Benchmark Generation for TCC Experiments.

This script executes a manual, statically defined configuration matrix derived
from experimental saturation boundaries.

All generated benchmark instances are stored directly into the './raw/' directory
to maintain structural alignment with the discovery framework artifacts.
"""

import os
import importlib

TARGET_OUTPUT_DIR = "raw"

# Dict structure: "problem_tag": ("python_module_name", [list_of_explicit_N_values])
BENCHMARK_MATRIX = {
    "coi_killer": ("coi_killer", [2, 4, 8, 16, 24, 32, 48, 64, 96, 128]),
    "bad_order": ("bad_order", [5, 10, 20, 40, 60, 80, 100, 116, 122, 127]),
    "counter": ("counter", [2, 4, 8, 12, 16, 20, 22, 24, 25, 26]),
    "rule30": ("rule30", [3, 6, 12, 18, 24, 30, 36, 39, 42, 47]),
    "brp2": ("brp2", [3, 6, 12, 24, 36, 48, 60, 72, 78, 83]),
    "dining": ("dining", [2, 4, 8, 12, 16, 24, 32, 40, 44, 46]),
    "elevator2": ("elevator2", [3, 4, 6, 8, 9, 10, 12, 15, 18, 19]),
    "fischer": ("fischer", [3, 4, 5, 6, 7, 8]),
    "firewire_link": ("firewire_link", [2, 4, 8, 12, 16, 24, 32, 36, 38, 39]),
    "mcs": ("mcs", [3, 4, 5]),
    "bakery": ("bakery", [2, 3, 4]),
    "train_gate": ("train_gate", [3, 4, 5, 6, 7, 8, 9]),
    "synapse": ("synapse", [3, 4, 5, 6, 7, 8]),
    "sokoban": ("sokoban", [2, 3, 4, 5])
}

def main():
    print("=== Launching Clean Experimental Suite Generation ===")
    print(f"Target persistent storage directory: ./{TARGET_OUTPUT_DIR}/\n")

    # Safely verify that target directory exists before file stream dumping
    os.makedirs(TARGET_OUTPUT_DIR, exist_ok=True)

    generated_count = 0
    failed_count = 0

    for problem_tag, (module_name, n_values) in BENCHMARK_MATRIX.items():
        print(f"⚡ Scaling instances for category: [{problem_tag.upper()}]")

        # Dynamic module loading over python files context
        try:
            mod = importlib.import_module(module_name)
        except ImportError as e:
            print(f"  ❌ Error: Unable to import generator module '{module_name}': {e}")
            failed_count += len(n_values)
            continue

        for n in n_values:
            print(f"   -> Instantiating N = {n} ... ", end="", flush=True)
            try:
                # Calls the uniform module generation layout function
                ssmv_content = mod.generate_instance(n)

                # Output path maps explicitly inside raw/ folder context
                out_filename = f"{module_name}_{n}.smv"
                out_path = os.path.join(TARGET_OUTPUT_DIR, out_filename)

                with open(out_path, "w") as f:
                    f.write(ssmv_content)

                print("SUCCESS")
                generated_count += 1
            except Exception as e:
                print("FAILED")
                print(f"      >> Error trace: {e}")
                failed_count += 1

    print("\n================================================================================")
    print("=== Suite Production Complete ===")
    print(f"  - Successfully created: {generated_count} progressive .smv models.")
    print(f"  - Failed/Skipped:        {failed_count} instances.")
    print(f"  - Repository assets securely stored under: ./{TARGET_OUTPUT_DIR}/")
    print("================================================================================")

if __name__ == "__main__":
    main()
