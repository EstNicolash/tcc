#!/usr/bin/env python3
"""
run_all_benchmarks_master.py
================================================================================
Orquestrador Mestre de Benchmarks para o ArsVera.

Varre em lote todos os catálogos de problemas pendentes com suas respectivas
escalas de N, invocando o pipeline experimental concorrente de forma autônoma.
"""

import sys
import time
from pathlib import Path

# Garante a importação da função principal do seu ecossistema experimental
try:
    from run_benchmarks import execute_benchmark_suite
except ImportError:
    print("❌ Erro: O script 'run_experiments_suite.py' precisa estar no mesmo diretório.")
    sys.exit(1)

# ─── CATALOGAÇÃO DE MODELOS E ESCALAS DE CONFIGURAÇÃO ─────────────────────────
# Removidos: dining (já coletado), coi_killer, coi_chaos_killer e firewire_link.
EXPERIMENTAL_CATALOG = {
    #"bad_order":   [5, 15, 30, 60, 90, 104, 112, 118, 122, 125, 127],
    #"counter":     [2, 4, 8, 12, 16, 18, 20, 22, 24, 25, 26],
    #"rule30":      [3, 6, 12, 18, 24, 30, 36, 39, 41, 44, 47],
    #"brp2":        [3, 6, 12, 24, 36, 48, 60, 72, 76, 80, 83],
    #"elevator2":   [3, 6, 9, 10, 12, 14, 16, 17, 18, 19],
    #"fischer":     [3, 4, 5, 6, 7, 8],
    "synapse":     [3, 4, 5, 6, 7, 8],
    "train_gate":  [3, 4, 5, 6, 7, 8, 9],
    "mcs":         [3, 4, 5],
    "bakery":      [2, 3, 4],
    "sokoban":     [2, 3, 4, 5]
}

# ─── PARÂMETROS GLOBAIS DE EXECUÇÃO ───────────────────────────────────────────
HEURISTICS = ["default", "random:42", "force:500", "dynamic"]
ALGORITHMS = ["bdd", "labelling-scc"]
ITERATIONS = 30  # Mantido suas 30 amostras estatísticas por grade

def main():
    start_suite = time.time()
    total_problems = len(EXPERIMENTAL_CATALOG)

    print("================================================================================")
    print("   🌐 INICIALIZANDO SPRINT DE BENCHMARKS AUTOMATIZADO — MOTOR ARSVERA    ")
    print("================================================================================")
    print(f"Instâncias agendadas para processamento: {list(EXPERIMENTAL_CATALOG.keys())}")
    print(f"Configurações: Heurísticas={HEURISTICS} | Algoritmos={ALGORITHMS} | Iterações={ITERATIONS}")
    print("================================================================================")

    for idx, (problem_name, n_scales) in enumerate(EXPERIMENTAL_CATALOG.items(), start=1):
        iter_start = time.time()
        print(f"\n[{idx}/{total_problems}] 🟢 Iniciando Lote de Problemas: >>> {problem_name.upper()} <<<")
        print(f"Escalas de N alvo mapeadas: {n_scales}")

        try:
            # Invoca o core do seu framework reaproveitando a lógica de cross-killing e podas por saturação
            execute_benchmark_suite(
                problem_name=problem_name,
                heuristics=HEURISTICS,
                iterations=ITERATIONS,
                algorithms=ALGORITHMS
            )

            elapsed = time.time() - iter_start
            print(f"🏁 Lote [{problem_name.upper()}] concluído com sucesso em {elapsed:.2f}s.")

        except Exception as e:
            print(f"❌ Falha crítica catastrófica ao processar o lote [{problem_name.upper()}]: {e}")
            print("Saltando para o próximo catálogo para não interromper a suíte...")
            continue

    total_elapsed = time.time() - start_suite
    print("\n================================================================================")
    print(f"🎉 SUÍTE GLOBAL DE EXPERIMENTOS CONCLUÍDA EM: {total_elapsed/3600:.2f} HORAS.")
    print("Todos os relatórios CSV isolados encontram-se na pasta './benchmark/'.")
    print("================================================================================")

if __name__ == "__main__":
    main()
