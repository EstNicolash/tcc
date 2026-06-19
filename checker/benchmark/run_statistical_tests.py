#!/usr/bin/env python3
"""
run_statistical_tests.py
================================================================================
Advanced Statistical Analysis Pipeline for Model Checking Paradigms.

Features:
  - Agnostic Automated Directory Discovery (Locates latest benchmarks)
  - Monolithic Data Sanitization & Mapping
  - Point-to-Point Local Contrast (Mann-Whitney U)
  - Full-Curve Global Contrast (Two-Sample Kolmogorov-Smirnov)
  - Multidimensional Internal Heuristics Tournament via Wilcoxon Signed-Rank Test
  - Strict Alpha Adjustment with Expanded Bonferroni Correction (Time, Memory, Nodes)
"""

import os
import sys
import glob
import itertools
import numpy as np
import pandas as pd
from scipy import stats

# ==================== CONFIGURAÇÃO DO DIRETÓRIO ====================
OUT_DIR = "resultados_estatisticos_txt"
# ===================================================================

def discover_latest_benchmark_csvs() -> dict:
    """Varre o diretório local de forma agnóstica descobrindo o CSV mais recente por problema."""
    if os.path.exists("benchmarks_dining.csv") or "run_statistical_tests.py" in os.listdir("."):
        base_dir = "."
    elif os.path.exists("benchmark"):
        base_dir = "benchmark"
    else:
        base_dir = "."

    print(f"🔍 Varrendo o diretório '{os.path.abspath(base_dir)}' em busca de benchmarks...")

    try:
        all_subdirs = [os.path.join(base_dir, d) for d in os.listdir(base_dir) if os.path.isdir(os.path.join(base_dir, d))]
    except Exception as e:
        print(f"❌ Erro ao acessar o diretório: {e}")
        return {}

    problem_groups = {}
    for d in all_subdirs:
        folder_name = os.path.basename(d)
        if folder_name in ["resultados_estatisticos_txt", "estatisticos", "txt", "__pycache__"] or folder_name.startswith("resultados_"):
            continue

        if "_results_" in folder_name:
            problem_name = folder_name.split("_results_")[0].strip().lower()
            if problem_name not in problem_groups:
                problem_groups[problem_name] = []
            problem_groups[problem_name].append(d)

    chosen_csvs = {}
    for problem_name, paths in problem_groups.items():
        paths.sort()
        latest_dir = paths[-1]

        csv_token = "synapses" if problem_name in ["synapse", "synapses"] else problem_name
        csv_files = glob.glob(os.path.join(latest_dir, f"benchmarks_{csv_token}.csv"))
        if not csv_files:
            csv_files = glob.glob(os.path.join(latest_dir, "*.csv"))

        if csv_files:
            chosen_csvs[problem_name] = csv_files[0]
            print(f"  -> {problem_name:14s}: Selecionado -> {os.path.basename(latest_dir)}/{os.path.basename(csv_files[0])}")

    return chosen_csvs

def load_and_sanitize_data(file_path: str) -> pd.DataFrame:
    """Carrega os dados, limpa os tipos e padroniza as nomenclaturas dos motores."""
    df = pd.read_csv(file_path)

    name_map = {
        'SSMV-BDD': 'llull-BDD',
        'NuSMV-Standard': 'NuSMV-static',
        'NuSMV-DynamicSifting': 'NuSMV-dynamic',
        'NuSMV-StaticImport': 'NuSMV-static',
        'SSMV-LABELLING-SCC': 'llull-labelling'
    }
    if 'Algorithm' in df.columns:
        df['Algorithm'] = df['Algorithm'].map(name_map).fillna(df['Algorithm'])

    if 'Heuristic' in df.columns:
        df['Heuristic'] = df['Heuristic'].astype(str).str.split(':').str[0].str.lower()

    numeric_cols = ['Total_MS', 'Compile_MS', 'Verify_MS', 'Static_Nodes', 'Verification_Nodes', 'States_Explored', 'Peak_Mem_KB']
    for col in numeric_cols:
        if col in df.columns:
            df[col] = pd.to_numeric(df[col], errors='coerce')

    return df

def generate_statistical_summary(df: pd.DataFrame) -> pd.DataFrame:
    """Gera uma tabela agrupada com média, desvio padrão e contagem de runs de sucesso."""
    df_success = df[df['Status'] == 'SUCCESS']
    if df_success.empty:
        return pd.DataFrame(columns=['N', 'Heuristic', 'Algorithm', 'Media_Total_MS', 'Desvio_Total_MS', 'Runs_Sucesso'])

    summary = df_success.groupby(['N', 'Heuristic', 'Algorithm'])['Total_MS'].agg(
        Media_Total_MS='mean',
        Desvio_Total_MS='std',
        Runs_Sucesso='count'
    ).reset_index()

    summary['Media_Total_MS'] = summary['Media_Total_MS'].round(2)
    summary['Desvio_Total_MS'] = summary['Desvio_Total_MS'].round(2)
    return summary

def run_local_and_global_tests(df: pd.DataFrame, heuristic_filter: str, ssmv_alg: str, nusmv_alg: str):
    """Executa testes não-paramétricos locais (Mann-Whitney U) e globais (KS) para todas as métricas."""
    df_filtered = df[(df['Heuristic'] == heuristic_filter) & (df['Status'] == 'SUCCESS')].copy()

    data_ssmv = df_filtered[df_filtered['Algorithm'] == ssmv_alg]
    data_nusmv = df_filtered[df_filtered['Algorithm'] == nusmv_alg]

    if data_ssmv.empty or data_nusmv.empty:
        return

    ns_comuns = sorted(list(set(data_ssmv['N'].dropna().unique()).intersection(set(data_nusmv['N'].dropna().unique()))))

    if not ns_comuns:
        return

    print(f"\n=======================================================================")
    print(f"📊 CONFRONTO ESTATÍSTICO SEPARADO (Heurística: {heuristic_filter.upper()}): {ssmv_alg} vs {nusmv_alg}")
    print(f"=======================================================================")

    metrics_to_test = [
        ('Total_MS', 'TEMPO TOTAL (MS)', 'ms'),
        ('Compile_MS', 'TEMPO DE COMPILAÇÃO (MS)', 'ms'),
        ('Verify_MS', 'TEMPO DE VERIFICAÇÃO (MS)', 'ms'),
        ('Peak_Mem_KB', 'PICO DE MEMÓRIA RAM (KB)', 'KB'),
        ('Verification_Nodes', 'QUANTIDADE DE NÓS ATIVOS NO BDD', 'nós')
    ]

    # --- 1. CONFRONTO LOCAL: MANN-WHITNEY U PONTO A PONTO ---
    for metric, label, unit in metrics_to_test:
        print(f"\n➔ Confronto Local Ponto a Ponto (Mann-Whitney U) - {label}:")
        for n in ns_comuns:
            g_ssmv = data_ssmv[data_ssmv['N'] == n][metric].dropna().values
            g_nusmv = data_nusmv[data_nusmv['N'] == n][metric].dropna().values

            if metric == 'Verification_Nodes' and (ssmv_alg == 'llull-labelling' or nusmv_alg == 'llull-labelling'):
                continue

            if len(g_ssmv) > 0 and len(g_nusmv) > 0:
                try:
                    _, p_u = stats.mannwhitneyu(g_ssmv, g_nusmv, alternative='two-sided')
                    med_ssmv, med_nusmv = np.median(g_ssmv), np.median(g_nusmv)

                    status = "Diferença Signif." if p_u < 0.05 else "Empate Estatístico"
                    vencedor = ssmv_alg if (med_ssmv < med_nusmv and p_u < 0.05) else (nusmv_alg if (med_nusmv < med_ssmv and p_u < 0.05) else "Nenhum")

                    print(f"  N = {n:2d} | Mediana llull: {med_ssmv:9.2f} {unit} | Mediana NuSMV: {med_nusmv:9.2f} {unit} | p-value: {p_u:.5f} -> {status} (Ganhou: {vencedor})")
                except ValueError:
                    print(f"  N = {n:2d} | ⚠️ Dados insuficientes (variância zero) para computar Mann-Whitney U.")

    # --- 2. CONFRONTO GLOBAL: KOLMOGOROV-SMIRNOV ---
    print(f"\n➔ Confronto Global da Curva Completa (Kolmogorov-Smirnov bicaudal):")
    for metric, label, _ in metrics_to_test:
        v_ssmv = data_ssmv[metric].dropna().values
        v_nusmv = data_nusmv[metric].dropna().values

        if metric == 'Verification_Nodes' and (ssmv_alg == 'llull-labelling' or nusmv_alg == 'llull-labelling'):
            continue

        if len(v_ssmv) > 0 and len(v_nusmv) > 0:
            stat_ks, p_ks = stats.ks_2samp(v_ssmv, v_nusmv)
            status_ks = 'Diferença Global Relevante' if p_ks < 0.005 else 'Curvas Globalmente Equivalentes'
            print(f"  > {label:36s} | Distância D: {stat_ks:.4f} | p-value: {p_ks:.5e} -> {status_ks}")

def run_internal_heuristics_wilcoxon_tournament(df: pd.DataFrame, target_algorithm: str):
    """
    Executa confrontos pareados internos entre as heurísticas do mesmo motor
    utilizando o Teste de Wilcoxon com Correção de Bonferroni expandido
    para Tempo, Memória e Nós do BDD.
    """
    df_alg = df[(df['Algorithm'] == target_algorithm) & (df['Status'] == 'SUCCESS')].copy()
    if df_alg.empty:
        return

    heuristics = sorted([h for h in df_alg['Heuristic'].unique() if h in ['default', 'force', 'dynamic']])
    if len(heuristics) < 2:
        return

    metrics_to_test = [
        ('Verify_MS', 'TEMPO DE VERIFICAÇÃO', 'ms'),
        ('Peak_Mem_KB', 'PICO DE MEMÓRIA RAM', 'KB'),
        ('Verification_Nodes', 'QUANTIDADE DE NÓS NO BDD', 'nós')
    ]

    pairs = list(itertools.combinations(heuristics, 2))

    # Correção de Bonferroni Expandida: 3 métricas x N pares de heurísticas
    total_hypotheses = len(metrics_to_test) * len(pairs)
    alpha_original = 0.05
    alpha_bonferroni = alpha_original / total_hypotheses

    print(f"\n=======================================================================")
    print(f"⚔️ TORNEIO MULTIDIMENSIONAL DE HEURÍSTICAS VIA WILCOXON (Motor: {target_algorithm})")
    print(f" Hipóteses Simultâneas: {total_hypotheses} | Alfa de Bonferroni Corrigido: {alpha_bonferroni:.6f}")
    print(f"=======================================================================")

    for metric, label, unit in metrics_to_test:
        if metric == 'Verification_Nodes' and 'labelling' in target_algorithm:
            continue

        print(f"\n➔ Confrontos para a dimensão: {label}:")

        # Agrupa por N e extrai a mediana para o emparelhamento estrito dos vetores
        pivot_df = df_alg.groupby(['N', 'Heuristic'])[metric].median().unstack()
        pivot_clean = pivot_df[heuristics].dropna()

        if len(pivot_clean) < 4:
            print(f"   ⚠️ Instâncias pareadas insuficientes ({len(pivot_clean)}) para avaliar {label}.")
            continue

        for h1, h2 in pairs:
            v1 = pivot_clean[h1].values
            v2 = pivot_clean[h2].values

            if np.allclose(v1, v2, atol=1e-2):
                print(f"   ↳ {h1.upper():7s} vs {h2.upper():7s} | ⚠️ Curvas identicamente coladas. Empate absoluto.")
                continue

            try:
                stat, p_w = stats.wilcoxon(v1, v2, alternative='two-sided')

                if p_w < alpha_bonferroni:
                    status = "Diferença Estatística Significativa"
                    med_h1, med_h2 = np.median(v1), np.median(v2)
                    # Para as 3 métricas, o menor valor indica maior eficiência computacional/física
                    vencedor = h1 if med_h1 < med_h2 else h2
                    print(f"   ↳ {h1.upper():7s} vs {h2.upper():7s} | p-value: {p_w:.5f} -> {status} (Melhor: {vencedor.upper()})")
                else:
                    print(f"   ↳ {h1.upper():7s} vs {h2.upper():7s} | p-value: {p_w:.5f} -> Equivalência Estatística (Empate)")

            except Exception as e:
                print(f"   ↳ {h1.upper():7s} vs {h2.upper():7s} | ⚠️ Falha ao computar teste: {e}")

# ==================== EXECUÇÃO DO BATCH PIPELINE ====================
if __name__ == "__main__":
    os.makedirs(OUT_DIR, exist_ok=True)
    target_csvs = discover_latest_benchmark_csvs()

    if not target_csvs:
        print(f"\n❌ Erro: Nenhuma pasta no formato 'nome_results_data' foi localizada.")
        sys.exit(1)

    print(f"\n🚀 Iniciando processamento estatístico para {len(target_csvs)} instâncias...")

    for problem_name, csv_path in target_csvs.items():
        print(f"  ➔ Analisando dinamicamente heurísticas para: '{problem_name}'...")
        txt_log_path = os.path.join(OUT_DIR, f"analise_estatistica_{problem_name}.txt")

        try:
            raw_data = load_and_sanitize_data(csv_path)
            heuristics_found = raw_data[raw_data['Status'] == 'SUCCESS']['Heuristic'].dropna().unique()

            with open(txt_log_path, "w", encoding="utf-8") as f_out:
                original_stdout = sys.stdout
                sys.stdout = f_out

                try:
                    print(f"=======================================================================")
                    print(f" Pipeline de Relatório Estatístico Monolítico: {problem_name.upper()}")
                    print(f"=======================================================================")

                    summary_table = generate_statistical_summary(raw_data)
                    print("\n--- [Tabela Resumo de Médias de Tempo Total] ---")
                    if not summary_table.empty:
                        print(summary_table.to_string(index=False))
                    else:
                        print("Empty DataFrame (Nenhum sucesso registrado para este problema).")

                    # 1. Dispara os confrontos externos cruzados entre motores distintos
                    for heur in sorted(heuristics_found):
                        run_local_and_global_tests(raw_data, heur, 'llull-BDD', 'NuSMV-static')
                        run_local_and_global_tests(raw_data, heur, 'llull-BDD', 'NuSMV-dynamic')

                    # 2. Dispara o torneio interno tridimensional entre heurísticas do mesmo motor
                    print("\n" + "#"*71 + "\n")
                    run_internal_heuristics_wilcoxon_tournament(raw_data, 'llull-BDD')
                    run_internal_heuristics_wilcoxon_tournament(raw_data, 'NuSMV-static')

                finally:
                    sys.stdout = original_stdout

        except Exception as e:
            print(f"  ❌ Falha crítica ao tentar processar {problem_name}: {str(e)}")

    print(f"\n✨ Concluído! Todos os relatórios dinâmicos foram salvos na pasta: '{os.path.abspath(OUT_DIR)}/'")
